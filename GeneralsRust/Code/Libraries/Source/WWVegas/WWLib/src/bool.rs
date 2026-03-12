//! Legacy bool compatibility helpers mirroring WWLib `bool.h`.

/// Compatibility TRUE constant.
pub const TRUE: bool = true;
/// Compatibility FALSE constant.
pub const FALSE: bool = false;

/// Legacy Bool alias.
pub type Bool = bool;

/// Integer-style bool for serialization.
#[repr(i32)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BoolInt {
    False = 0,
    True = 1,
}

impl From<bool> for BoolInt {
    fn from(value: bool) -> Self {
        if value {
            BoolInt::True
        } else {
            BoolInt::False
        }
    }
}

impl From<BoolInt> for bool {
    fn from(value: BoolInt) -> Self {
        matches!(value, BoolInt::True)
    }
}
