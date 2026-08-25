//! Compiler and portability helpers mirroring WWLib `always.h`.

use core::cmp::{max, min};

/// Mirror C++ NULL macro.
pub const NULL: usize = 0;

/// Inline hint comparable to MSVC __forceinline.
#[macro_export]
macro_rules! ww_inline {
    ($($t:tt)*) => {
        #[inline(always)]
        $($t)*
    };
}

/// Max helper (C++ macro replacement).
#[inline]
pub fn ww_max<T: PartialOrd + Copy>(a: T, b: T) -> T {
    if a > b { a } else { b }
}

/// Min helper (C++ macro replacement).
#[inline]
pub fn ww_min<T: PartialOrd + Copy>(a: T, b: T) -> T {
    if a < b { a } else { b }
}

/// Return array size (for fixed-size arrays).
#[macro_export]
macro_rules! array_size {
    ($arr:expr) => {
        ($arr).len()
    };
}

/// Size of a field in a struct.
#[macro_export]
macro_rules! size_of_field {
    ($ty:ty, $field:tt) => {{
        let uninit = core::mem::MaybeUninit::<$ty>::uninit();
        let ptr = uninit.as_ptr();
        let field_ptr = unsafe { core::ptr::addr_of!((*ptr).$field) };
        core::mem::size_of_val(unsafe { &*field_ptr })
    }};
}

/// Marker trait mirroring the W3DMPO base class.
pub trait W3dMpo {
    fn glue_enforcer(&self) -> usize;
}

/// Use standard `min`/`max` for integer types in Rust code.
#[inline]
pub fn max_i32(a: i32, b: i32) -> i32 {
    max(a, b)
}

#[inline]
pub fn min_i32(a: i32, b: i32) -> i32 {
    min(a, b)
}
