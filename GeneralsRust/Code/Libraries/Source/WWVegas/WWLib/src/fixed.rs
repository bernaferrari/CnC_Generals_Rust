// Auto-generated C++ compatibility shim for fixed-point
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Fixed(pub i32);

impl Fixed {
    pub const SCALE: i32 = 1 << 16;

    pub fn from_float(value: f32) -> Self {
        Self((value * Self::SCALE as f32) as i32)
    }

    pub fn to_float(self) -> f32 {
        self.0 as f32 / Self::SCALE as f32
    }
}

impl std::ops::Add for Fixed {
    type Output = Self;
    fn add(self, rhs: Self) -> Self {
        Self(self.0 + rhs.0)
    }
}

impl std::ops::Sub for Fixed {
    type Output = Self;
    fn sub(self, rhs: Self) -> Self {
        Self(self.0 - rhs.0)
    }
}

impl std::ops::Mul for Fixed {
    type Output = Self;
    fn mul(self, rhs: Self) -> Self {
        let v = (self.0 as i64 * rhs.0 as i64) >> 16;
        Self(v as i32)
    }
}

impl std::ops::Div for Fixed {
    type Output = Self;
    fn div(self, rhs: Self) -> Self {
        let v = ((self.0 as i64) << 16) / rhs.0 as i64;
        Self(v as i32)
    }
}
