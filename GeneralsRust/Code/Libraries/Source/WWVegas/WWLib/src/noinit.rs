//! No-init constructor marker mirroring WWLib `noinit.h`.
//!
//! This class is solely used as a parameter to a constructor that does
//! absolutely no initialization to the object being constructed. By using
//! this method, it is possible to load and save data directly from a
//! class that has virtual functions. The construction process automatically
//! takes care of initializing the virtual function table pointer and the
//! rest of the constructor doesn't initialize any data members. After loading
//! into a class object, simply perform an in-place new operation.

/// Marker type used to signal a no-init constructor path.
///
/// # Examples
///
/// ```rust
/// use wwlib::noinit::NoInit;
///
/// struct MyStruct {
///     x: i32,
///     y: i32,
/// }
///
/// impl MyStruct {
///     /// Normal constructor - initializes fields
///     fn new() -> Self {
///         MyStruct { x: 0, y: 0 }
///     }
///
///     /// No-init constructor - leaves fields uninitialized (for load/save)
///     fn new_no_init(_marker: NoInit) -> Self {
///         unsafe { std::mem::MaybeUninit::uninit().assume_init() }
///     }
/// }
/// ```
#[derive(Clone, Copy, Debug)]
pub struct NoInit;

impl NoInit {
    /// Create a new NoInit marker instance.
    pub const fn new() -> Self {
        NoInit
    }

    /// Call operator equivalent - does nothing, matching C++ `operator()`.
    pub fn call(&self) {}
}

impl Default for NoInit {
    fn default() -> Self {
        NoInit
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_no_init_creation() {
        let marker = NoInit;
        assert!(std::mem::size_of::<NoInit>() == 0);
    }

    #[test]
    fn test_no_init_call() {
        let marker = NoInit;
        marker.call(); // Should do nothing
    }

    #[test]
    fn test_no_init_default() {
        let marker = NoInit::default();
        marker.call();
    }

    #[test]
    fn test_no_init_copy_clone() {
        let marker1 = NoInit;
        let marker2 = marker1;
        let marker3 = marker1;
        // All should be valid
        marker1.call();
        marker2.call();
        marker3.call();
    }
}
