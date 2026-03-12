//! Trig utilities (C++ compatibility).
//!
//! Matches Libraries/Include/Lib/trig.h.

use base_types::Real;

#[inline]
pub fn Sin(x: Real) -> Real {
    x.sin()
}

#[inline]
pub fn Cos(x: Real) -> Real {
    x.cos()
}

#[inline]
pub fn Tan(x: Real) -> Real {
    x.tan()
}

#[inline]
pub fn ACos(x: Real) -> Real {
    x.acos()
}

#[inline]
pub fn ASin(x: Real) -> Real {
    x.asin()
}
