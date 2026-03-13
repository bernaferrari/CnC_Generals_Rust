//! Swap utility mirroring WWLib `swap.h`.
//!
//! Provides a generic swap function that exchanges the values of two objects,
//! matching the behavior of the C++ template `swap<T>(T&, T&)` from WWLib.

/// Swaps the values of two mutable references.
///
/// This mirrors the C++ template function from `swap.h`:
/// ```cpp
/// template<class T>
/// void swap(T & left, T & right) {
///     T temp;
///     temp = left;
///     left = right;
///     right = temp;
/// }
/// ```
///
/// The Rust implementation uses `std::mem::swap` which achieves identical
/// observable behavior while being safe and efficient.
#[inline]
pub fn swap<T>(a: &mut T, b: &mut T) {
    std::mem::swap(a, b);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_swap_integers() {
        let mut a = 10;
        let mut b = 20;
        swap(&mut a, &mut b);
        assert_eq!(a, 20);
        assert_eq!(b, 10);
    }

    #[test]
    fn test_swap_floats() {
        let mut a = 1.5f64;
        let mut b = 2.5f64;
        swap(&mut a, &mut b);
        assert_eq!(a, 2.5);
        assert_eq!(b, 1.5);
    }

    #[test]
    fn test_swap_strings() {
        let mut a = String::from("hello");
        let mut b = String::from("world");
        swap(&mut a, &mut b);
        assert_eq!(a, "world");
        assert_eq!(b, "hello");
    }

    #[test]
    fn test_swap_same_value() {
        let mut a = 42;
        let mut b = 42;
        swap(&mut a, &mut b);
        assert_eq!(a, 42);
        assert_eq!(b, 42);
    }

    #[test]
    fn test_swap_arrays() {
        let mut a = [1, 2, 3];
        let mut b = [4, 5, 6];
        swap(&mut a, &mut b);
        assert_eq!(a, [4, 5, 6]);
        assert_eq!(b, [1, 2, 3]);
    }

    #[test]
    fn test_swap_tuples() {
        let mut a = (1, "a");
        let mut b = (2, "b");
        swap(&mut a, &mut b);
        assert_eq!(a, (2, "b"));
        assert_eq!(b, (1, "a"));
    }
}
