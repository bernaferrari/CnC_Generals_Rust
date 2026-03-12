//! Core WWMath utilities and constants.
//!
//! This module provides the fundamental mathematical constants and utility functions
//! that are used throughout the WWMath library, converted from the original C++ wwmath.h.

use std::f32;
use std::sync::LazyLock;
use std::sync::Mutex;

/// Mathematical constants from the original WWMath library
pub const EPSILON: f32 = 0.0001f32;
pub const EPSILON2: f32 = EPSILON * EPSILON;
pub const PI: f32 = std::f32::consts::PI;
pub const FLOAT_MAX: f32 = f32::MAX;
pub const FLOAT_MIN: f32 = f32::MIN;
pub const SQRT2: f32 = std::f32::consts::SQRT_2;
pub const SQRT3: f32 = 1.732_050_8_f32;
pub const OO_SQRT2: f32 = std::f32::consts::FRAC_1_SQRT_2;
pub const OO_SQRT3: f32 = 1.0_f32 / SQRT3;

/// Conversion macros between degrees and radians
pub const RAD_TO_DEG: f32 = 180.0f32 / PI;
pub const DEG_TO_RAD: f32 = PI / 180.0f32;

/// Table sizes for fast trigonometry functions
pub const ARC_TABLE_SIZE: usize = 1024;
pub const SIN_TABLE_SIZE: usize = 1024;

/// Lookup tables for fast trigonometry
static FAST_ACOS_TABLE: LazyLock<Mutex<Vec<f32>>> =
    LazyLock::new(|| Mutex::new(vec![0.0; ARC_TABLE_SIZE]));
static FAST_ASIN_TABLE: LazyLock<Mutex<Vec<f32>>> =
    LazyLock::new(|| Mutex::new(vec![0.0; ARC_TABLE_SIZE]));
static FAST_SIN_TABLE: LazyLock<Mutex<Vec<f32>>> =
    LazyLock::new(|| Mutex::new(vec![0.0; SIN_TABLE_SIZE]));
static FAST_INV_SIN_TABLE: LazyLock<Mutex<Vec<f32>>> =
    LazyLock::new(|| Mutex::new(vec![0.0; SIN_TABLE_SIZE]));
static INITIALIZED: LazyLock<Mutex<bool>> = LazyLock::new(|| Mutex::new(false));

/// Collection of mathematical utility functions
pub struct WWMath;

impl WWMath {
    /// Public constant accessors matching the legacy C++ API.
    pub const EPSILON: f32 = EPSILON;
    pub const EPSILON2: f32 = EPSILON2;
    pub const PI: f32 = PI;
    pub const FLOAT_MAX: f32 = FLOAT_MAX;
    pub const FLOAT_MIN: f32 = FLOAT_MIN;
    pub const SQRT2: f32 = SQRT2;
    pub const SQRT3: f32 = SQRT3;
    pub const OO_SQRT2: f32 = OO_SQRT2;
    pub const OO_SQRT3: f32 = OO_SQRT3;

    /// Initialize the WWMath subsystem and precompute lookup tables
    pub fn init() {
        let mut initialized = INITIALIZED.lock().unwrap();
        if *initialized {
            return;
        }

        // Initialize arc tables
        {
            let mut acos_table = FAST_ACOS_TABLE.lock().unwrap();
            let mut asin_table = FAST_ASIN_TABLE.lock().unwrap();

            for a in 0..ARC_TABLE_SIZE {
                let cv = (a as f32 - ARC_TABLE_SIZE as f32 / 2.0)
                    * (1.0 / (ARC_TABLE_SIZE as f32 / 2.0));
                acos_table[a] = cv.acos();
                asin_table[a] = cv.asin();
            }
        }

        // Initialize sin tables
        {
            let mut sin_table = FAST_SIN_TABLE.lock().unwrap();
            let mut inv_sin_table = FAST_INV_SIN_TABLE.lock().unwrap();

            for a in 0..SIN_TABLE_SIZE {
                let cv = a as f32 * 2.0 * PI / SIN_TABLE_SIZE as f32;
                sin_table[a] = cv.sin();

                if a > 0 {
                    inv_sin_table[a] = 1.0 / sin_table[a];
                } else {
                    inv_sin_table[a] = FLOAT_MAX;
                }
            }
        }

        *initialized = true;
    }

    /// Shutdown the WWMath subsystem
    pub fn shutdown() {
        // In Rust, we don't need to explicitly cleanup as the tables will be
        // automatically dropped when the program exits
    }
    /// Fast absolute value for f32 using bit manipulation
    pub fn fabs(val: f32) -> f32 {
        // Convert to bits, mask out sign bit, convert back
        let bits = val.to_bits();
        let abs_bits = bits & 0x7FFF_FFFF;
        f32::from_bits(abs_bits)
    }

    /// Fast check if float is positive using bit manipulation
    pub fn fast_is_float_positive(val: f32) -> bool {
        // Check sign bit (bit 31)
        (val.to_bits() & 0x8000_0000) == 0
    }

    /// Check if a number is a power of 2
    pub fn is_power_of_2(val: u32) -> bool {
        val != 0 && (val & (val - 1)) == 0
    }

    /// Clamp a float value to a range
    pub fn clamp(val: f32, min: f32, max: f32) -> f32 {
        val.max(min).min(max)
    }

    /// Clamp a double value to a range
    pub fn clamp_double(val: f64, min: f64, max: f64) -> f64 {
        val.max(min).min(max)
    }

    /// Clamp an integer value to a range
    pub fn clamp_int(val: i32, min_val: i32, max_val: i32) -> i32 {
        val.max(min_val).min(max_val)
    }

    /// Wrap a float value to a range (handles periodic values)
    pub fn wrap(val: f32, min: f32, max: f32) -> f32 {
        let range = max - min;
        if range == 0.0 {
            return min;
        }

        let mut value = val;
        while value >= max {
            value -= range;
        }
        while value < min {
            value += range;
        }

        value.clamp(min, max)
    }

    /// Wrap a double value to a range (handles periodic values)
    pub fn wrap_double(val: f64, min: f64, max: f64) -> f64 {
        let range = max - min;
        if range == 0.0 {
            return min;
        }

        let mut value = val;
        while value >= max {
            value -= range;
        }
        while value < min {
            value += range;
        }

        value.clamp(min, max)
    }

    /// Get the minimum of two floats
    pub fn min(a: f32, b: f32) -> f32 {
        a.min(b)
    }

    /// Get the maximum of two floats
    pub fn max(a: f32, b: f32) -> f32 {
        a.max(b)
    }

    /// Linear interpolation between two floats
    pub fn lerp(a: f32, b: f32, lerp_factor: f32) -> f32 {
        a + (b - a) * lerp_factor
    }

    /// Linear interpolation between two doubles
    pub fn lerp_double(a: f64, b: f64, lerp_factor: f32) -> f64 {
        a + (b - a) * lerp_factor as f64
    }

    /// Convert float to int with truncation towards zero
    pub fn float_to_int_chop(f: f32) -> i32 {
        if !f.is_finite() {
            return 0;
        }

        if f >= i32::MAX as f32 {
            i32::MAX
        } else if f <= i32::MIN as f32 {
            i32::MIN
        } else {
            f.trunc() as i32
        }
    }

    /// Convert float to int with floor behavior
    pub fn float_to_int_floor(f: f32) -> i32 {
        if !f.is_finite() {
            return 0;
        }

        if f >= i32::MAX as f32 {
            i32::MAX
        } else if f <= i32::MIN as f32 {
            i32::MIN
        } else {
            f.floor() as i32
        }
    }

    /// Convert float to long with truncation
    pub fn float_to_long(f: f32) -> i64 {
        f as i64
    }

    /// Convert double to long with truncation
    pub fn double_to_long(f: f64) -> i64 {
        f as i64
    }

    /// Get the sign of a float (-1, 0, or 1)
    pub fn sign(val: f32) -> f32 {
        if val > 0.0 {
            1.0
        } else if val < 0.0 {
            -1.0
        } else {
            0.0
        }
    }

    /// Cosine function
    pub fn cos(val: f32) -> f32 {
        val.cos()
    }

    /// Sine function
    pub fn sin(val: f32) -> f32 {
        val.sin()
    }

    /// Square root function
    pub fn sqrt(val: f32) -> f32 {
        val.sqrt()
    }

    /// Fast inverse square root using the famous Quake III algorithm
    /// About 30% faster than regular square root + division
    pub fn inv_sqrt(val: f32) -> f32 {
        // The famous "fast inverse square root" from Quake III
        // Ported to Rust with proper bit manipulation

        let i = val.to_bits();
        let i = 0x5f37_59df - (i >> 1); // Magic number for f32
        let y = f32::from_bits(i);

        // One iteration of Newton-Raphson refinement
        y * (1.5 - (val * 0.5 * y * y))
    }

    /// Arc tangent of y/x (f64 version)
    pub fn atan2(y: f64, x: f64) -> f64 {
        y.atan2(x)
    }

    /// Arc tangent of y/x (f32 version)
    pub fn atan2f(y: f32, x: f32) -> f32 {
        y.atan2(x)
    }

    /// Arc tangent
    pub fn atan(x: f32) -> f32 {
        x.atan()
    }

    /// Arc cosine
    pub fn acos(val: f32) -> f32 {
        val.acos()
    }

    /// Arc sine
    pub fn asin(val: f32) -> f32 {
        val.asin()
    }

    /// Ceil function
    pub fn ceil(val: f32) -> f32 {
        val.ceil()
    }

    /// Floor function
    pub fn floor(val: f32) -> f32 {
        val.floor()
    }

    /// Generate a random float between 0 and 1
    pub fn random_float() -> f32 {
        Self::random_float_improved()
    }

    /// Generate a random float between 0 and 1 (inclusive of 0, exclusive of 1)
    pub fn unit_random() -> f32 {
        Self::random_float_improved()
    }

    /// Generate a random float in a range
    pub fn random_float_range(min: f32, max: f32) -> f32 {
        Self::random_float() * (max - min) + min
    }

    /// Convert float to byte (0-255)
    pub fn unit_float_to_byte(f: f32) -> u8 {
        (f * 255.0).clamp(0.0, 255.0) as u8
    }

    /// Convert byte (0-255) to unit float (0-1)
    pub fn byte_to_unit_float(byte: u8) -> f32 {
        byte as f32 / 255.0
    }

    /// Check if float is valid (not NaN or infinite)
    pub fn is_valid_float(x: f32) -> bool {
        !x.is_nan() && !x.is_infinite()
    }

    /// Check if double is valid (not NaN or infinite)
    pub fn is_valid_double(x: f64) -> bool {
        !x.is_nan() && !x.is_infinite()
    }

    /// Fast table-based sine function
    pub fn fast_sin(val: f32) -> f32 {
        Self::ensure_initialized();

        let index = val * SIN_TABLE_SIZE as f32 / (2.0 * PI);

        let idx0 = Self::float_to_int_floor(index) as usize;
        let idx1 = idx0 + 1;
        let frac = index - idx0 as f32;

        let idx0 = idx0 & (SIN_TABLE_SIZE - 1);
        let idx1 = idx1 & (SIN_TABLE_SIZE - 1);

        let sin_table = FAST_SIN_TABLE.lock().unwrap();
        (1.0 - frac) * sin_table[idx0] + frac * sin_table[idx1]
    }

    /// Fast table-based cosine function
    pub fn fast_cos(val: f32) -> f32 {
        Self::fast_sin(val + PI * 0.5)
    }

    /// Fast table-based inverse sine function
    pub fn fast_inv_sin(val: f32) -> f32 {
        // For now, fall back to division to avoid precision issues near 0
        1.0 / Self::fast_sin(val)
    }

    /// Fast table-based inverse cosine function
    pub fn fast_inv_cos(val: f32) -> f32 {
        1.0 / Self::fast_cos(val)
    }

    /// Fast table-based arc cosine function
    pub fn fast_acos(val: f32) -> f32 {
        Self::ensure_initialized();

        // Near -1 and +1, the table becomes too inaccurate
        if Self::fabs(val) > 0.975 {
            return Self::acos(val);
        }

        let scaled_val = val * (ARC_TABLE_SIZE as f32 / 2.0);

        let idx0 = Self::float_to_int_floor(scaled_val) as usize;
        let idx1 = idx0 + 1;
        let frac = scaled_val - idx0 as f32;

        let idx0 = idx0 + ARC_TABLE_SIZE / 2;
        let idx1 = idx1 + ARC_TABLE_SIZE / 2;

        // Bounds check
        if idx0 >= ARC_TABLE_SIZE || idx1 >= ARC_TABLE_SIZE {
            return Self::acos(val);
        }

        let acos_table = FAST_ACOS_TABLE.lock().unwrap();
        (1.0 - frac) * acos_table[idx0] + frac * acos_table[idx1]
    }

    /// Fast table-based arc sine function
    pub fn fast_asin(val: f32) -> f32 {
        Self::ensure_initialized();

        // Near -1 and +1, the table becomes too inaccurate
        if Self::fabs(val) > 0.975 {
            return Self::asin(val);
        }

        let scaled_val = val * (ARC_TABLE_SIZE as f32 / 2.0);

        let idx0 = Self::float_to_int_floor(scaled_val) as usize;
        let idx1 = idx0 + 1;
        let frac = scaled_val - idx0 as f32;

        let idx0 = idx0 + ARC_TABLE_SIZE / 2;
        let idx1 = idx1 + ARC_TABLE_SIZE / 2;

        // Bounds check
        if idx0 >= ARC_TABLE_SIZE || idx1 >= ARC_TABLE_SIZE {
            return Self::asin(val);
        }

        let asin_table = FAST_ASIN_TABLE.lock().unwrap();
        (1.0 - frac) * asin_table[idx0] + frac * asin_table[idx1]
    }

    /// Convert radians to degrees
    pub fn rad_to_deg(rad: f32) -> f32 {
        rad * RAD_TO_DEG
    }

    /// Convert degrees to radians
    pub fn deg_to_rad(deg: f32) -> f32 {
        deg * DEG_TO_RAD
    }

    /// Convert radians to degrees (f64 version)
    pub fn rad_to_deg_double(rad: f64) -> f64 {
        rad * RAD_TO_DEG as f64
    }

    /// Convert degrees to radians (f64 version)
    pub fn deg_to_rad_double(deg: f64) -> f64 {
        deg * DEG_TO_RAD as f64
    }

    /// Better random number generator matching C++ behavior
    pub fn random_float_improved() -> f32 {
        // Use a simple LCG similar to the C++ rand() & 0xFFF approach
        use std::cell::RefCell;

        thread_local! {
            static RNG_STATE: RefCell<u32> = const { RefCell::new(1) };
        }

        RNG_STATE.with(|state| {
            let mut s = state.borrow_mut();
            // Linear congruential generator
            *s = s.wrapping_mul(1103515245).wrapping_add(12345);
            ((*s & 0xFFF) as f32) / 0xFFF as f32
        })
    }

    /// Ensure lookup tables are initialized
    fn ensure_initialized() {
        let initialized = INITIALIZED.lock().unwrap();
        if !*initialized {
            drop(initialized);
            Self::init();
        }
    }

    /// Convert float to its bit representation as int
    pub fn float_as_int(f: f32) -> i32 {
        f.to_bits() as i32
    }
}

/// Euler angle conversion orders
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum EulerOrder {
    // Static axes
    XYZs,
    XYXs,
    XZYs,
    XZXs,
    YZXs,
    YZYs,
    YXZs,
    YXYs,
    ZXYs,
    ZXZs,
    ZYXs,
    ZYZs,
    // Rotating axes
    XYZr,
    XYXr,
    XZYr,
    XZXr,
    YZXr,
    YZYr,
    YXZr,
    YXYr,
    ZXYr,
    ZXZr,
    ZYXr,
    ZYZr,
}

const EULER_FRAME_STATIC: u32 = 0x0000_0000;
const EULER_FRAME_ROTATING: u32 = 0x0000_0001;
const EULER_REPEAT_NO: u32 = 0x0000_0000;
const EULER_REPEAT_YES: u32 = 0x0000_0001;
const EULER_PARITY_EVEN: u32 = 0x0000_0000;
const EULER_PARITY_ODD: u32 = 0x0000_0001;

impl EulerOrder {
    fn to_raw_order(self) -> u32 {
        match self {
            // Static axes
            EulerOrder::XYZs => {
                Self::build_order(0, EULER_PARITY_EVEN, EULER_REPEAT_NO, EULER_FRAME_STATIC)
            }
            EulerOrder::XYXs => {
                Self::build_order(0, EULER_PARITY_EVEN, EULER_REPEAT_YES, EULER_FRAME_STATIC)
            }
            EulerOrder::XZYs => {
                Self::build_order(0, EULER_PARITY_ODD, EULER_REPEAT_NO, EULER_FRAME_STATIC)
            }
            EulerOrder::XZXs => {
                Self::build_order(0, EULER_PARITY_ODD, EULER_REPEAT_YES, EULER_FRAME_STATIC)
            }
            EulerOrder::YZXs => {
                Self::build_order(1, EULER_PARITY_EVEN, EULER_REPEAT_NO, EULER_FRAME_STATIC)
            }
            EulerOrder::YZYs => {
                Self::build_order(1, EULER_PARITY_EVEN, EULER_REPEAT_YES, EULER_FRAME_STATIC)
            }
            EulerOrder::YXZs => {
                Self::build_order(1, EULER_PARITY_ODD, EULER_REPEAT_NO, EULER_FRAME_STATIC)
            }
            EulerOrder::YXYs => {
                Self::build_order(1, EULER_PARITY_ODD, EULER_REPEAT_YES, EULER_FRAME_STATIC)
            }
            EulerOrder::ZXYs => {
                Self::build_order(2, EULER_PARITY_EVEN, EULER_REPEAT_NO, EULER_FRAME_STATIC)
            }
            EulerOrder::ZXZs => {
                Self::build_order(2, EULER_PARITY_EVEN, EULER_REPEAT_YES, EULER_FRAME_STATIC)
            }
            EulerOrder::ZYXs => {
                Self::build_order(2, EULER_PARITY_ODD, EULER_REPEAT_NO, EULER_FRAME_STATIC)
            }
            EulerOrder::ZYZs => {
                Self::build_order(2, EULER_PARITY_ODD, EULER_REPEAT_YES, EULER_FRAME_STATIC)
            }
            // Rotating axes
            EulerOrder::ZYXr => {
                Self::build_order(0, EULER_PARITY_EVEN, EULER_REPEAT_NO, EULER_FRAME_ROTATING)
            }
            EulerOrder::XYXr => {
                Self::build_order(0, EULER_PARITY_EVEN, EULER_REPEAT_YES, EULER_FRAME_ROTATING)
            }
            EulerOrder::YZXr => {
                Self::build_order(0, EULER_PARITY_ODD, EULER_REPEAT_NO, EULER_FRAME_ROTATING)
            }
            EulerOrder::XZXr => {
                Self::build_order(0, EULER_PARITY_ODD, EULER_REPEAT_YES, EULER_FRAME_ROTATING)
            }
            EulerOrder::XZYr => {
                Self::build_order(1, EULER_PARITY_EVEN, EULER_REPEAT_NO, EULER_FRAME_ROTATING)
            }
            EulerOrder::YZYr => {
                Self::build_order(1, EULER_PARITY_EVEN, EULER_REPEAT_YES, EULER_FRAME_ROTATING)
            }
            EulerOrder::ZXYr => {
                Self::build_order(1, EULER_PARITY_ODD, EULER_REPEAT_NO, EULER_FRAME_ROTATING)
            }
            EulerOrder::YXYr => {
                Self::build_order(1, EULER_PARITY_ODD, EULER_REPEAT_YES, EULER_FRAME_ROTATING)
            }
            EulerOrder::YXZr => {
                Self::build_order(2, EULER_PARITY_EVEN, EULER_REPEAT_NO, EULER_FRAME_ROTATING)
            }
            EulerOrder::ZXZr => {
                Self::build_order(2, EULER_PARITY_EVEN, EULER_REPEAT_YES, EULER_FRAME_ROTATING)
            }
            EulerOrder::XYZr => {
                Self::build_order(2, EULER_PARITY_ODD, EULER_REPEAT_NO, EULER_FRAME_ROTATING)
            }
            EulerOrder::ZYZr => {
                Self::build_order(2, EULER_PARITY_ODD, EULER_REPEAT_YES, EULER_FRAME_ROTATING)
            }
        }
    }

    fn build_order(i: u32, p: u32, r: u32, f: u32) -> u32 {
        (((((i << 1) + p) << 1) + r) << 1) + f
    }
}

/// Euler angles class for matrix/angle conversions
#[derive(Debug, Clone)]
pub struct EulerAngles {
    angles: [f64; 3],
    order: EulerOrder,
}

/// Matrix4x4 type alias for compatibility with C++ WW3D
pub type Matrix4x4 = crate::matrix4::Matrix4;

/// Matrix3x3 type alias for compatibility with C++ WW3D
pub type Matrix3x3 = crate::matrix3::Matrix3;

/// Additional type aliases for C++ compatibility
pub type Vector3 = crate::vector3::Vector3;
pub type Vector4 = crate::vector4::Vector4;
pub type Vector2 = crate::vector2::Vector2;
pub type Quaternion = crate::quat::Quaternion;

impl Default for EulerAngles {
    fn default() -> Self {
        Self {
            angles: [0.0; 3],
            order: EulerOrder::XYZr,
        }
    }
}

impl EulerAngles {
    /// Create new EulerAngles with default values
    pub fn new() -> Self {
        Self::default()
    }

    /// Create EulerAngles from a 3x3 matrix
    pub fn from_matrix(matrix: &[[f32; 3]; 3], order: EulerOrder) -> Self {
        let mut result = Self {
            angles: [0.0; 3],
            order,
        };
        result.set_from_matrix_impl(matrix);
        result
    }

    /// Get the angle at index i
    pub fn get_angle(&self, i: usize) -> f64 {
        self.angles[i]
    }

    /// Convert matrix to euler angles
    fn set_from_matrix_impl(&mut self, matrix: &[[f32; 3]; 3]) {
        let order = self.order.to_raw_order();
        let (i, j, k, _h, n, s, f) = Self::unpack_order(order);

        if s == EULER_REPEAT_YES {
            let sy = (matrix[i][j] * matrix[i][j] + matrix[i][k] * matrix[i][k]).sqrt();

            if sy > 16.0 * f32::EPSILON {
                self.angles[0] = WWMath::atan2(matrix[i][j] as f64, matrix[i][k] as f64);
                self.angles[1] = WWMath::atan2(sy as f64, matrix[i][i] as f64);
                self.angles[2] = WWMath::atan2(matrix[j][i] as f64, -matrix[k][i] as f64);
            } else {
                self.angles[0] = WWMath::atan2(-matrix[j][k] as f64, matrix[j][j] as f64);
                self.angles[1] = WWMath::atan2(sy as f64, matrix[i][i] as f64);
                self.angles[2] = 0.0;
            }
        } else {
            let cy = (matrix[i][i] * matrix[i][i] + matrix[j][i] * matrix[j][i]).sqrt();

            if cy > 16.0 * f32::EPSILON {
                self.angles[0] = WWMath::atan2(matrix[k][j] as f64, matrix[k][k] as f64);
                self.angles[1] = WWMath::atan2(-matrix[k][i] as f64, cy as f64);
                self.angles[2] = WWMath::atan2(matrix[j][i] as f64, matrix[i][i] as f64);
            } else {
                self.angles[0] = WWMath::atan2(-matrix[j][k] as f64, matrix[j][j] as f64);
                self.angles[1] = WWMath::atan2(-matrix[k][i] as f64, cy as f64);
                self.angles[2] = 0.0;
            }
        }

        if n == EULER_PARITY_ODD {
            self.angles[0] = -self.angles[0];
            self.angles[1] = -self.angles[1];
            self.angles[2] = -self.angles[2];
        }

        if f == EULER_FRAME_ROTATING {
            self.angles.swap(0, 2);
        }

        // Special cleanup for XYZr order
        if matches!(self.order, EulerOrder::XYZr) {
            self.cleanup_xyzr();
        }
    }

    /// Convert euler angles to a 3x3 matrix
    pub fn to_matrix(&self) -> [[f32; 3]; 3] {
        let mut matrix = [[0.0f32; 3]; 3];

        // Initialize as identity
        matrix[0][0] = 1.0;
        matrix[1][1] = 1.0;
        matrix[2][2] = 1.0;

        let mut a0 = self.angles[0];
        let mut a1 = self.angles[1];
        let mut a2 = self.angles[2];

        let order = self.order.to_raw_order();
        let (i, j, k, _h, n, s, f) = Self::unpack_order(order);

        if f == EULER_FRAME_ROTATING {
            std::mem::swap(&mut a0, &mut a2);
        }

        if n == EULER_PARITY_ODD {
            a0 = -a0;
            a1 = -a1;
            a2 = -a2;
        }

        let ci = a0.cos();
        let cj = a1.cos();
        let ch = a2.cos();
        let si = a0.sin();
        let sj = a1.sin();
        let sh = a2.sin();

        let cc = ci * ch;
        let cs = ci * sh;
        let sc = si * ch;
        let ss = si * sh;

        if s == EULER_REPEAT_YES {
            matrix[i][i] = cj as f32;
            matrix[i][j] = (sj * si) as f32;
            matrix[i][k] = (sj * ci) as f32;
            matrix[j][i] = (sj * sh) as f32;
            matrix[j][j] = (-cj * ss + cc) as f32;
            matrix[j][k] = (-cj * cs - sc) as f32;
            matrix[k][i] = (-sj * ch) as f32;
            matrix[k][j] = (cj * sc + cs) as f32;
            matrix[k][k] = (cj * cc - ss) as f32;
        } else {
            matrix[i][i] = (cj * ch) as f32;
            matrix[i][j] = (sj * sc - cs) as f32;
            matrix[i][k] = (sj * cc + ss) as f32;
            matrix[j][i] = (cj * sh) as f32;
            matrix[j][j] = (sj * ss + cc) as f32;
            matrix[j][k] = (sj * cs - sc) as f32;
            matrix[k][i] = (-sj) as f32;
            matrix[k][j] = (cj * si) as f32;
            matrix[k][k] = (cj * ci) as f32;
        }

        matrix
    }

    fn unpack_order(order: u32) -> (usize, usize, usize, usize, u32, u32, u32) {
        const EULER_SAFE: [usize; 4] = [0, 1, 2, 0];
        const EULER_NEXT: [usize; 4] = [1, 2, 0, 1];

        let f = order & 1;
        let order = order >> 1;
        let s = order & 1;
        let order = order >> 1;
        let n = order & 1;
        let order = order >> 1;

        let i = EULER_SAFE[order as usize & 3];
        let j = EULER_NEXT[i + n as usize];
        let k = EULER_NEXT[i + 1 - n as usize];
        let h = if s == 1 { k } else { i };

        (i, j, k, h, n, s, f)
    }

    fn cleanup_xyzr(&mut self) {
        const PI: f64 = std::f64::consts::PI;

        let mut x2 = PI + self.angles[0];
        let mut y2 = PI - self.angles[1];
        let mut z2 = PI + self.angles[2];

        if x2 > PI {
            x2 -= 2.0 * PI;
        }
        if y2 > PI {
            y2 -= 2.0 * PI;
        }
        if z2 > PI {
            z2 -= 2.0 * PI;
        }

        let mag0 = self.angles[0] * self.angles[0]
            + self.angles[1] * self.angles[1]
            + self.angles[2] * self.angles[2];
        let mag1 = x2 * x2 + y2 * y2 + z2 * z2;

        if mag1 < mag0 {
            self.angles[0] = x2;
            self.angles[1] = y2;
            self.angles[2] = z2;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::f32::consts::PI;

    #[test]
    fn test_constants() {
        assert!((EPSILON - 0.0001f32).abs() < 1e-10);
        assert!((PI - std::f32::consts::PI).abs() < 1e-10);
        assert!((SQRT2 - std::f32::consts::SQRT_2).abs() < 1e-10);
    }

    #[test]
    fn test_fabs() {
        assert_eq!(WWMath::fabs(5.0), 5.0);
        assert_eq!(WWMath::fabs(-5.0), 5.0);
        assert_eq!(WWMath::fabs(0.0), 0.0);
    }

    #[test]
    fn test_clamp() {
        assert_eq!(WWMath::clamp(5.0, 0.0, 10.0), 5.0);
        assert_eq!(WWMath::clamp(-5.0, 0.0, 10.0), 0.0);
        assert_eq!(WWMath::clamp(15.0, 0.0, 10.0), 10.0);
    }

    #[test]
    fn test_lerp() {
        assert_eq!(WWMath::lerp(0.0, 10.0, 0.5), 5.0);
        assert_eq!(WWMath::lerp(0.0, 10.0, 0.0), 0.0);
        assert_eq!(WWMath::lerp(0.0, 10.0, 1.0), 10.0);
    }

    #[test]
    fn test_sign() {
        assert_eq!(WWMath::sign(5.0), 1.0);
        assert_eq!(WWMath::sign(-5.0), -1.0);
        assert_eq!(WWMath::sign(0.0), 0.0);
    }

    #[test]
    fn test_is_power_of_2() {
        assert!(WWMath::is_power_of_2(1));
        assert!(WWMath::is_power_of_2(2));
        assert!(WWMath::is_power_of_2(4));
        assert!(WWMath::is_power_of_2(8));
        assert!(!WWMath::is_power_of_2(3));
        assert!(!WWMath::is_power_of_2(6));
        assert!(!WWMath::is_power_of_2(0));
    }

    #[test]
    fn test_wrap() {
        assert_eq!(WWMath::wrap(5.0, 0.0, 10.0), 5.0);
        assert_eq!(WWMath::wrap(15.0, 0.0, 10.0), 5.0);
        assert_eq!(WWMath::wrap(-5.0, 0.0, 10.0), 5.0);
    }

    #[test]
    fn test_min_max() {
        assert_eq!(WWMath::min(5.0, 10.0), 5.0);
        assert_eq!(WWMath::max(5.0, 10.0), 10.0);
    }

    #[test]
    fn test_is_valid_float() {
        assert!(WWMath::is_valid_float(5.0));
        assert!(WWMath::is_valid_float(0.0));
        assert!(WWMath::is_valid_float(-5.0));
        assert!(!WWMath::is_valid_float(f32::NAN));
        assert!(!WWMath::is_valid_float(f32::INFINITY));
        assert!(!WWMath::is_valid_float(f32::NEG_INFINITY));
    }

    #[test]
    fn test_fast_inv_sqrt() {
        // Test fast inverse square root against standard version
        let test_vals = [1.0, 4.0, 9.0, 16.0, 25.0, 0.25, 0.01];

        for &val in &test_vals {
            let fast_result = WWMath::inv_sqrt(val);
            let std_result = 1.0 / val.sqrt();
            let error = (fast_result - std_result).abs();

            // The fast version should be close but not exactly equal
            assert!(
                error < 0.01,
                "Fast inv_sqrt error too large for {}: {} vs {}",
                val,
                fast_result,
                std_result
            );
        }
    }

    #[test]
    fn test_fast_trig_functions() {
        WWMath::init(); // Ensure tables are initialized

        let test_angles = [0.0, PI / 6.0, PI / 4.0, PI / 3.0, PI / 2.0, PI];

        for &angle in &test_angles {
            let fast_sin = WWMath::fast_sin(angle);
            let std_sin = angle.sin();
            let sin_error = (fast_sin - std_sin).abs();

            let fast_cos = WWMath::fast_cos(angle);
            let std_cos = angle.cos();
            let cos_error = (fast_cos - std_cos).abs();

            // Fast trig should be reasonably accurate
            assert!(
                sin_error < 0.01,
                "Fast sin error too large for {}: {} vs {}",
                angle,
                fast_sin,
                std_sin
            );
            assert!(
                cos_error < 0.01,
                "Fast cos error too large for {}: {} vs {}",
                angle,
                fast_cos,
                std_cos
            );
        }
    }

    #[test]
    fn test_angle_conversion() {
        let deg = 90.0;
        let rad = WWMath::deg_to_rad(deg);
        let back_to_deg = WWMath::rad_to_deg(rad);

        assert!((rad - PI / 2.0).abs() < EPSILON);
        assert!((back_to_deg - deg).abs() < EPSILON);
    }

    #[test]
    fn test_euler_angles() {
        // Test identity matrix conversion
        let identity = [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]];

        let euler = EulerAngles::from_matrix(&identity, EulerOrder::XYZr);

        // All angles should be near zero for identity matrix
        assert!(euler.get_angle(0).abs() < 0.01);
        assert!(euler.get_angle(1).abs() < 0.01);
        assert!(euler.get_angle(2).abs() < 0.01);

        // Test round-trip conversion
        let reconstructed = euler.to_matrix();

        for i in 0..3 {
            for j in 0..3 {
                let error = (reconstructed[i][j] - identity[i][j]).abs();
                assert!(
                    error < 0.01,
                    "Matrix reconstruction error at [{}, {}]: {} vs {}",
                    i,
                    j,
                    reconstructed[i][j],
                    identity[i][j]
                );
            }
        }
    }

    #[test]
    fn test_float_to_int_conversion() {
        assert_eq!(WWMath::float_to_int_chop(3.7), 3);
        assert_eq!(WWMath::float_to_int_chop(-3.7), -3);
        assert_eq!(WWMath::float_to_int_floor(3.7), 3);
        assert_eq!(WWMath::float_to_int_floor(-3.7), -4);
    }

    #[test]
    fn test_random_generation() {
        // Test that random values are in expected range
        for _ in 0..100 {
            let val = WWMath::random_float_improved();
            assert!(
                val >= 0.0 && val <= 1.0,
                "Random value out of range: {}",
                val
            );
        }

        let min = 5.0;
        let max = 10.0;
        for _ in 0..100 {
            let val = WWMath::random_float_range(min, max);
            assert!(
                val >= min && val <= max,
                "Random range value out of bounds: {}",
                val
            );
        }
    }
}
