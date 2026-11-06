//! `cMathUtil` — GeneralsMD/Code/Libraries/Source/WWVegas/Wwutil/mathutil.{h,cpp}
//!
//! Intentional deviations from C++:
//! - PDF helpers take `&mut impl Rng` instead of process-wide `::rand()`, so two
//!   game instances can keep independent streams without a mutable global.
//! - Assertions that used `WWASSERT` become debug `debug_assert!`.

use crate::MISCUTIL_EPSILON;

/// Matches `cMathUtil::PI_1` (truncated literal from C++, not `f64::consts::PI`).
#[allow(clippy::approx_constant)]
pub const PI_1: f64 = 3.141_592_7;
/// Matches `cMathUtil::PI_2`.
#[allow(clippy::approx_constant)]
pub const PI_2: f64 = 1.570_796_3;

const WW_MATH_EPSILON: f64 = 0.0001;

/// Namespace mirroring C++ `cMathUtil` static methods.
pub struct MathUtil;

/// Minimal RNG surface used by the PDF helpers (avoids pulling rand into the API).
pub trait PdfRng {
    /// Uniform `f64` in `0.0..=1.0` (C++ `rand() / RAND_MAX`).
    fn next_unit_f64(&mut self) -> f64;
    /// Uniform `u32` for integer PDF (C++ `rand()`).
    fn next_u32(&mut self) -> u32;
}

impl PdfRng for fastrand::Rng {
    fn next_unit_f64(&mut self) -> f64 {
        self.f64()
    }
    fn next_u32(&mut self) -> u32 {
        self.u32(..)
    }
}

impl MathUtil {
    /// `cMathUtil::Angle_To_Vector` — degrees in, unit vector out (Windows Y flip).
    pub fn angle_to_vector(angle: f64) -> (f64, f64) {
        debug_assert!(angle > -WW_MATH_EPSILON && angle < 360.0 + WW_MATH_EPSILON);

        let (dx, mut dy) = if (0.0..90.0).contains(&angle) {
            let r = angle * PI_1 / 180.0;
            (r.sin(), r.cos())
        } else if (90.0..180.0).contains(&angle) {
            let r = (angle - 90.0) * PI_1 / 180.0;
            (r.cos(), -r.sin())
        } else if (180.0..270.0).contains(&angle) {
            let r = (angle - 180.0) * PI_1 / 180.0;
            (-r.sin(), -r.cos())
        } else {
            let r = (angle - 270.0) * PI_1 / 180.0;
            (-r.cos(), r.sin())
        };

        let len = (dx * dx + dy * dy).sqrt();
        debug_assert!((len - 1.0).abs() < 0.0005);
        let _ = len;
        // Correction for Irish nature of windows y coords
        dy *= -1.0;
        (dx, dy)
    }

    /// `cMathUtil::Vector_To_Angle` — returns degrees.
    pub fn vector_to_angle(dx: f64, dy: f64) -> f64 {
        let theta = if dx == 0.0 && dy == 0.0 {
            0.0
        } else if dx == 0.0 {
            if dy <= 0.0 {
                0.0
            } else {
                PI_1
            }
        } else {
            let mut t = (-dy / dx).atan();
            if dx < 0.0 {
                t += PI_1;
            }
            t += 3.0 * PI_2;
            if t >= 2.0 * PI_1 {
                t -= 2.0 * PI_1;
            }
            t = 2.0 * PI_1 - t;
            if t == 2.0 * PI_1 {
                t = 0.0;
            }
            t
        };
        theta * 180.0 / PI_1
    }

    /// `cMathUtil::Simple_Distance`
    pub fn simple_distance(x1: f64, y1: f64, x2: f64, y2: f64) -> f64 {
        let dx = x2 - x1;
        let dy = y2 - y1;
        (dx * dx + dy * dy).sqrt()
    }

    /// `cMathUtil::Round`
    pub fn round(arg: f64) -> i32 {
        if arg > f64::from(MISCUTIL_EPSILON) {
            (arg + 0.5) as i32
        } else if arg < -f64::from(MISCUTIL_EPSILON) {
            (arg - 0.5) as i32
        } else {
            0
        }
    }

    /// `cMathUtil::Rotate_Vector` — `angle` in degrees.
    pub fn rotate_vector(vx: f64, vy: f64, angle: f64) -> (f64, f64) {
        let angle_radians = angle * PI_1 / 180.0;
        let c = angle_radians.cos();
        let s = angle_radians.sin();
        (vx * c - vy * s, vx * s + vy * c)
    }

    /// `cMathUtil::Get_Uniform_Pdf_Double`
    pub fn get_uniform_pdf_double(lower: f64, upper: f64, rng: &mut impl PdfRng) -> f64 {
        debug_assert!(upper - lower > -f64::from(MISCUTIL_EPSILON));
        let x = lower + rng.next_unit_f64() * (upper - lower);
        debug_assert!(x - lower > -f64::from(MISCUTIL_EPSILON) && upper - x > -f64::from(MISCUTIL_EPSILON));
        x
    }

    /// `cMathUtil::Get_Normalized_Uniform_Pdf_Double`
    pub fn get_normalized_uniform_pdf_double(rng: &mut impl PdfRng) -> f64 {
        Self::get_uniform_pdf_double(0.0, 1.0, rng)
    }

    /// `cMathUtil::Get_Uniform_Pdf_Int`
    pub fn get_uniform_pdf_int(lower: i32, upper: i32, rng: &mut impl PdfRng) -> i32 {
        debug_assert!(lower <= upper);
        let span = (upper - lower + 1) as u32;
        let x = lower + (rng.next_u32() % span) as i32;
        debug_assert!(x >= lower && upper >= x);
        x
    }

    /// `cMathUtil::Get_Hat_Pdf_Double`
    pub fn get_hat_pdf_double(lower: f64, upper: f64, rng: &mut impl PdfRng) -> f64 {
        debug_assert!(upper - lower > -f64::from(MISCUTIL_EPSILON));
        let x = if (upper - lower).abs() < f64::from(MISCUTIL_EPSILON) {
            lower
        } else {
            let dx = (upper - lower) / 2.0;
            let dy = 1.0 / dx;
            let m = dy / dx;
            let c = -m * lower;
            let mut x = Self::get_uniform_pdf_double(lower, lower + dx, rng);
            let y = Self::get_uniform_pdf_double(0.0, dy, rng);
            if y > m * x + c {
                x += dx;
            }
            x
        };
        debug_assert!(x - lower > -f64::from(MISCUTIL_EPSILON) && upper - x > -f64::from(MISCUTIL_EPSILON));
        x
    }

    /// `cMathUtil::Get_Normalized_Hat_Pdf_Double`
    pub fn get_normalized_hat_pdf_double(rng: &mut impl PdfRng) -> f64 {
        Self::get_hat_pdf_double(0.0, 1.0, rng)
    }

    /// `cMathUtil::Get_Hat_Pdf_Int`
    pub fn get_hat_pdf_int(lower: i32, upper: i32, rng: &mut impl PdfRng) -> i32 {
        Self::round(Self::get_hat_pdf_double(f64::from(lower), f64::from(upper), rng))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn angle_0_points_up_after_y_flip() {
        let (dx, dy) = MathUtil::angle_to_vector(0.0);
        assert!((dx - 0.0).abs() < 1e-6);
        // cos(0)=1 then y flip => -1
        assert!((dy - (-1.0)).abs() < 1e-6);
    }

    #[test]
    fn angle_90_points_right() {
        let (dx, dy) = MathUtil::angle_to_vector(90.0);
        assert!((dx - 1.0).abs() < 1e-6, "dx={dx}");
        assert!(dy.abs() < 1e-6, "dy={dy}");
    }

    #[test]
    fn round_trip_distance() {
        assert!((MathUtil::simple_distance(0.0, 0.0, 3.0, 4.0) - 5.0).abs() < 1e-9);
    }

    #[test]
    fn round_matches_cpp_epsilon_rule() {
        assert_eq!(MathUtil::round(1.4), 1);
        assert_eq!(MathUtil::round(1.6), 2);
        assert_eq!(MathUtil::round(-1.6), -2);
        assert_eq!(MathUtil::round(0.0), 0);
    }

    #[test]
    fn rotate_90_degrees() {
        // Uses cMathUtil::PI_1 (=3.1415927), so 90° is not bit-exact vs true π.
        let (x, y) = MathUtil::rotate_vector(1.0, 0.0, 90.0);
        assert!(x.abs() < 1e-5, "x={x}");
        assert!((y - 1.0).abs() < 1e-5, "y={y}");
    }

    #[test]
    fn uniform_pdf_stays_in_range() {
        let mut rng = fastrand::Rng::with_seed(1);
        for _ in 0..100 {
            let x = MathUtil::get_uniform_pdf_double(2.0, 5.0, &mut rng);
            assert!((2.0..=5.0).contains(&x));
            let i = MathUtil::get_uniform_pdf_int(2, 5, &mut rng);
            assert!((2..=5).contains(&i));
        }
    }
}
