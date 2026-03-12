//! Compiler and portability helpers mirroring WWLib `always.h`.

use core::alloc::Layout;
use core::cmp::{max, min};
use core::mem::{align_of, size_of};
use core::ptr;
use std::alloc::{alloc, dealloc};

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
    if a > b {
        a
    } else {
        b
    }
}

/// Min helper (C++ macro replacement).
#[inline]
pub fn ww_min<T: PartialOrd + Copy>(a: T, b: T) -> T {
    if a < b {
        a
    } else {
        b
    }
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

/// Simple W3D memory pool descriptor.
pub struct W3dMemPool {
    allocation_size: usize,
    #[allow(dead_code)]
    name: String,
}

/// Create a W3D memory pool.
pub fn create_w3d_mem_pool(pool_name: &str, allocation_size: usize) -> *mut W3dMemPool {
    let pool = W3dMemPool {
        allocation_size: allocation_size.max(1),
        name: pool_name.to_string(),
    };
    Box::into_raw(Box::new(pool))
}

/// Allocate memory from a pool.
pub unsafe fn allocate_from_w3d_mem_pool(pool: *mut W3dMemPool, allocation_size: usize) -> *mut u8 {
    if pool.is_null() {
        return ptr::null_mut();
    }
    let size = if allocation_size == 0 {
        (*pool).allocation_size
    } else {
        allocation_size.max(1)
    };
    let layout = Layout::from_size_align(size, align_of::<usize>()).unwrap_or_else(|_| {
        Layout::from_size_align(size_of::<usize>(), align_of::<usize>()).unwrap()
    });
    unsafe { alloc(layout) }
}

/// Allocate memory from a pool with message metadata (ignored, for API parity).
pub unsafe fn allocate_from_w3d_mem_pool_with_msg(
    pool: *mut W3dMemPool,
    allocation_size: usize,
    _msg: &str,
    _unused: i32,
) -> *mut u8 {
    unsafe { allocate_from_w3d_mem_pool(pool, allocation_size) }
}

/// Free memory back to a pool.
pub unsafe fn free_from_w3d_mem_pool(pool: *mut W3dMemPool, ptr_to_free: *mut u8) {
    if pool.is_null() || ptr_to_free.is_null() {
        return;
    }
    let size = (*pool).allocation_size.max(1);
    let layout = Layout::from_size_align(size, align_of::<usize>()).unwrap_or_else(|_| {
        Layout::from_size_align(size_of::<usize>(), align_of::<usize>()).unwrap()
    });
    unsafe { dealloc(ptr_to_free, layout) };
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
