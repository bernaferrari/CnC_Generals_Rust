//! Original WWMath Euler conversion and the six local-axis Rust conventions.

use crate::wwmath::Matrix3x3;
use conventions::{EulerAngles as WwEulerAngles, EulerOrder as WwEulerOrder};

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

impl EulerOrder {
    fn rotating_axes(self) -> WwEulerOrder {
        match self {
            Self::XYZ => WwEulerOrder::XYZr,
            Self::XZY => WwEulerOrder::XZYr,
            Self::YXZ => WwEulerOrder::YXZr,
            Self::YZX => WwEulerOrder::YZXr,
            Self::ZXY => WwEulerOrder::ZXYr,
            Self::ZYX => WwEulerOrder::ZYXr,
        }
    }
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

    /// Convert three successive local-axis rotations to a matrix.
    pub fn to_matrix(&self) -> Matrix3x3 {
        let [[a, b, c], [d, e, f], [g, h, i]] =
            WwEulerAngles::from_angles(self.angles, self.order.rotating_axes()).to_matrix();
        Matrix3x3::from_values(a, b, c, d, e, f, g, h, i)
    }

    /// Extract three successive local-axis rotations from the source matrix.
    pub fn from_matrix(matrix: &Matrix3x3, order: EulerOrder) -> Self {
        let rows = [
            matrix[0].to_array(),
            matrix[1].to_array(),
            matrix[2].to_array(),
        ];
        let angles = WwEulerAngles::from_matrix(&rows, order.rotating_axes());
        Self {
            angles: std::array::from_fn(|index| angles.get_angle(index)),
            order,
        }
    }
}

impl Default for EulerAngles {
    fn default() -> Self {
        Self::new(EulerOrder::XYZ)
    }
}

/// Static and rotating conventions encoded by C++ EulerAnglesClass.
pub(crate) mod conventions {
    use crate::wwmath::WWMath;

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

    impl Default for EulerAngles {
        fn default() -> Self {
            Self {
                angles: [0.0; 3],
                order: EulerOrder::XYZs,
            }
        }
    }

    impl EulerAngles {
        /// Create new EulerAngles with default values
        pub fn new() -> Self {
            Self::default()
        }

        pub(super) fn from_angles(angles: [f64; 3], order: EulerOrder) -> Self {
            Self { angles, order }
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
                    self.angles[0] = f64::from(WWMath::atan2f(matrix[i][j], matrix[i][k]));
                    self.angles[1] = f64::from(WWMath::atan2f(sy, matrix[i][i]));
                    self.angles[2] = f64::from(WWMath::atan2f(matrix[j][i], -matrix[k][i]));
                } else {
                    self.angles[0] = f64::from(WWMath::atan2f(-matrix[j][k], matrix[j][j]));
                    self.angles[1] = f64::from(WWMath::atan2f(sy, matrix[i][i]));
                    self.angles[2] = 0.0;
                }
            } else {
                let cy = (matrix[i][i] * matrix[i][i] + matrix[j][i] * matrix[j][i]).sqrt();

                if cy > 16.0 * f32::EPSILON {
                    self.angles[0] = f64::from(WWMath::atan2f(matrix[k][j], matrix[k][k]));
                    self.angles[1] = f64::from(WWMath::atan2f(-matrix[k][i], cy));
                    self.angles[2] = f64::from(WWMath::atan2f(matrix[j][i], matrix[i][i]));
                } else {
                    self.angles[0] = f64::from(WWMath::atan2f(-matrix[j][k], matrix[j][j]));
                    self.angles[1] = f64::from(WWMath::atan2f(-matrix[k][i], cy));
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

            let ci = f64::from(WWMath::cos(a0 as f32));
            let cj = f64::from(WWMath::cos(a1 as f32));
            let ch = f64::from(WWMath::cos(a2 as f32));
            let si = f64::from(WWMath::sin(a0 as f32));
            let sj = f64::from(WWMath::sin(a1 as f32));
            let sh = f64::from(WWMath::sin(a2 as f32));

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
            // Euler.cpp uses this rounded double for the XYZr alternate.
            const PI: f64 = 3_141_592_654.0 / 1_000_000_000.0;

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
}
