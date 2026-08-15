//! C++ `RefCountClass` / `RefCountPtr<T>` (WWLib `refcount.h`, `ref_ptr.h`).
//!
//! The historic COM-style `Add_Ref` / `Release_Ref` pointer is `std::sync::Arc`.
//! Save/load does **not** need raw pointer identity here: `WWSaveLoad` remaps
//! addresses as `usize` in `pointerremap.rs`, where `RefCountPtr<T>` is also
//! `Arc<T>`. No `unsafe` is required.

use std::sync::Arc;

/// C++ `RefCountClass` payload handle.
pub type RefCount<T> = Arc<T>;

/// C++ `RefCountPtr<T>` — shared ownership, not a raw COM pointer.
pub type RefCountPtr<T> = Arc<T>;

/// `Create_NEW` / `Create_Get`: take ownership of a freshly constructed value.
pub fn create_new<T>(value: T) -> RefCountPtr<T> {
    Arc::new(value)
}

/// `Create_Peek`: clone an existing shared handle (adds a ref).
pub fn create_peek<T>(existing: &RefCountPtr<T>) -> RefCountPtr<T> {
    Arc::clone(existing)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn create_new_shares_like_add_ref() {
        let a = create_new(7u32);
        let b = create_peek(&a);
        assert_eq!(*a, 7);
        assert_eq!(*b, 7);
        assert_eq!(Arc::strong_count(&a), 2);
    }
}
