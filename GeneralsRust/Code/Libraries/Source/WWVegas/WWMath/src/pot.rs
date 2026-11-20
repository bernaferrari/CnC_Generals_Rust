//! Power-of-Two (POT) utilities.
//!
//! This module provides functions for working with powers of two, including
//! finding the closest power of two to a given value and computing logarithms
//! base 2 for power-of-two values.
//!
//! These utilities are commonly used in graphics programming for texture sizes,
//! buffer allocations, and other scenarios where power-of-two values are required.

/// Find the closest inclusive power of 2 to a value.
///
/// This function finds the smallest power of 2 that is greater than or equal
/// to the input value. If the input is already a power of 2, it returns that value.
/// The original sign-bit result is retained when the answer is 2³¹.
/// Negative Rust inputs return 1 because the original signed loop cannot terminate.
///
/// # Arguments
/// * `val` - The input value to find the closest power of 2 for
///
/// # Returns
/// The smallest power of 2 that is >= `val`
///
/// # Examples
/// ```
/// use math_utilities::pot::find_pot;
///
/// assert_eq!(find_pot(1), 1);     // 2^0 = 1
/// assert_eq!(find_pot(2), 2);     // 2^1 = 2  
/// assert_eq!(find_pot(3), 4);     // 2^2 = 4
/// assert_eq!(find_pot(7), 8);     // 2^3 = 8
/// assert_eq!(find_pot(8), 8);     // 2^3 = 8 (already power of 2)
/// assert_eq!(find_pot(15), 16);   // 2^4 = 16
/// ```
pub fn find_pot(val: i32) -> i32 {
    if val <= 0 {
        return 1;
    }
    (val as u32).next_power_of_two() as i32
}

/// Find the log2 of the closest inclusive power of 2 to a value.
///
/// This function returns the exponent of the smallest power of 2 that is
/// greater than or equal to the input value. In other words, it returns
/// the value `n` such that `2^n` is the closest power of 2 to the input.
///
/// # Arguments
/// * `val` - The input value to find the closest power of 2 log for
///
/// # Returns
/// The log base 2 of the smallest power of 2 that is >= `val`
///
/// # Examples
/// ```
/// use math_utilities::pot::find_pot_log2;
///
/// assert_eq!(find_pot_log2(1), 0);     // 2^0 = 1
/// assert_eq!(find_pot_log2(2), 1);     // 2^1 = 2
/// assert_eq!(find_pot_log2(3), 2);     // 2^2 = 4
/// assert_eq!(find_pot_log2(7), 3);     // 2^3 = 8
/// assert_eq!(find_pot_log2(8), 3);     // 2^3 = 8 (already power of 2)
/// assert_eq!(find_pot_log2(15), 4);    // 2^4 = 16
/// ```
pub fn find_pot_log2(val: u32) -> u32 {
    u32::BITS - val.saturating_sub(1).leading_zeros()
}

/// Check if a number is a power of 2.
///
/// # Arguments
/// * `val` - The value to check
///
/// # Returns
/// `true` if the value is a power of 2, `false` otherwise
///
/// # Examples
/// ```
/// use math_utilities::pot::is_power_of_2;
///
/// assert_eq!(is_power_of_2(1), true);    // 2^0
/// assert_eq!(is_power_of_2(2), true);    // 2^1
/// assert_eq!(is_power_of_2(4), true);    // 2^2
/// assert_eq!(is_power_of_2(8), true);    // 2^3
/// assert_eq!(is_power_of_2(3), false);   // Not a power of 2
/// assert_eq!(is_power_of_2(5), false);   // Not a power of 2
/// assert_eq!(is_power_of_2(0), false);   // 0 is not a power of 2
/// ```
pub fn is_power_of_2(val: u32) -> bool {
    val.is_power_of_two()
}

/// Find the next power of 2 greater than the given value.
///
/// Unlike `find_pot`, this function always returns the next higher power of 2,
/// even if the input is already a power of 2.
/// The Rust compatibility extension accepts the original signed input range.
/// Larger input values return 1.
///
/// # Arguments
/// * `val` - The input value
///
/// # Returns
/// The next power of 2 greater than `val`
///
/// # Examples
/// ```
/// use math_utilities::pot::next_power_of_2;
///
/// assert_eq!(next_power_of_2(1), 2);     // Next power after 1 is 2
/// assert_eq!(next_power_of_2(2), 4);     // Next power after 2 is 4
/// assert_eq!(next_power_of_2(3), 4);     // Next power after 3 is 4
/// assert_eq!(next_power_of_2(8), 16);    // Next power after 8 is 16
/// ```
pub fn next_power_of_2(val: u32) -> u32 {
    let inclusive = find_pot(val as i32) as u32;
    if inclusive == val {
        inclusive << 1
    } else {
        inclusive
    }
}

/// Find the previous power of 2 less than or equal to the given value.
///
/// # Arguments
/// * `val` - The input value
///
/// # Returns
/// The largest power of 2 that is <= `val`
///
/// # Examples
/// ```
/// use math_utilities::pot::prev_power_of_2;
///
/// assert_eq!(prev_power_of_2(1), 1);     // 2^0
/// assert_eq!(prev_power_of_2(2), 2);     // 2^1
/// assert_eq!(prev_power_of_2(3), 2);     // 2^1
/// assert_eq!(prev_power_of_2(7), 4);     // 2^2
/// assert_eq!(prev_power_of_2(8), 8);     // 2^3
/// assert_eq!(prev_power_of_2(15), 8);    // 2^3
/// ```
pub fn prev_power_of_2(val: u32) -> u32 {
    val.checked_ilog2().map_or(0, |bit| 1 << bit)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_find_pot() {
        assert_eq!(find_pot(0), 1);
        assert_eq!(find_pot(1), 1);
        assert_eq!(find_pot(2), 2);
        assert_eq!(find_pot(3), 4);
        assert_eq!(find_pot(4), 4);
        assert_eq!(find_pot(5), 8);
        assert_eq!(find_pot(7), 8);
        assert_eq!(find_pot(8), 8);
        assert_eq!(find_pot(9), 16);
        assert_eq!(find_pot(15), 16);
        assert_eq!(find_pot(16), 16);
        assert_eq!(find_pot(17), 32);
        assert_eq!(find_pot(255), 256);
        assert_eq!(find_pot(256), 256);
        assert_eq!(find_pot(257), 512);
    }

    #[test]
    fn test_find_pot_log2() {
        assert_eq!(find_pot_log2(0), 0);
        assert_eq!(find_pot_log2(1), 0);
        assert_eq!(find_pot_log2(2), 1);
        assert_eq!(find_pot_log2(3), 2);
        assert_eq!(find_pot_log2(4), 2);
        assert_eq!(find_pot_log2(5), 3);
        assert_eq!(find_pot_log2(7), 3);
        assert_eq!(find_pot_log2(8), 3);
        assert_eq!(find_pot_log2(9), 4);
        assert_eq!(find_pot_log2(15), 4);
        assert_eq!(find_pot_log2(16), 4);
        assert_eq!(find_pot_log2(17), 5);
        assert_eq!(find_pot_log2(255), 8);
        assert_eq!(find_pot_log2(256), 8);
        assert_eq!(find_pot_log2(257), 9);
    }

    #[test]
    fn test_is_power_of_2() {
        assert!(!is_power_of_2(0));
        assert!(is_power_of_2(1));
        assert!(is_power_of_2(2));
        assert!(!is_power_of_2(3));
        assert!(is_power_of_2(4));
        assert!(!is_power_of_2(5));
        assert!(!is_power_of_2(6));
        assert!(!is_power_of_2(7));
        assert!(is_power_of_2(8));
        assert!(!is_power_of_2(9));
        assert!(!is_power_of_2(15));
        assert!(is_power_of_2(16));
        assert!(!is_power_of_2(17));
        assert!(is_power_of_2(256));
        assert!(!is_power_of_2(255));
        assert!(is_power_of_2(1024));
        assert!(!is_power_of_2(1023));
    }

    #[test]
    fn test_next_power_of_2() {
        assert_eq!(next_power_of_2(0), 1);
        assert_eq!(next_power_of_2(1), 2);
        assert_eq!(next_power_of_2(2), 4);
        assert_eq!(next_power_of_2(3), 4);
        assert_eq!(next_power_of_2(4), 8);
        assert_eq!(next_power_of_2(5), 8);
        assert_eq!(next_power_of_2(7), 8);
        assert_eq!(next_power_of_2(8), 16);
        assert_eq!(next_power_of_2(15), 16);
        assert_eq!(next_power_of_2(16), 32);
        assert_eq!(next_power_of_2(255), 256);
        assert_eq!(next_power_of_2(256), 512);
    }

    #[test]
    fn test_prev_power_of_2() {
        assert_eq!(prev_power_of_2(0), 0);
        assert_eq!(prev_power_of_2(1), 1);
        assert_eq!(prev_power_of_2(2), 2);
        assert_eq!(prev_power_of_2(3), 2);
        assert_eq!(prev_power_of_2(4), 4);
        assert_eq!(prev_power_of_2(5), 4);
        assert_eq!(prev_power_of_2(7), 4);
        assert_eq!(prev_power_of_2(8), 8);
        assert_eq!(prev_power_of_2(9), 8);
        assert_eq!(prev_power_of_2(15), 8);
        assert_eq!(prev_power_of_2(16), 16);
        assert_eq!(prev_power_of_2(17), 16);
        assert_eq!(prev_power_of_2(255), 128);
        assert_eq!(prev_power_of_2(256), 256);
        assert_eq!(prev_power_of_2(257), 256);
    }

    #[test]
    fn test_relationship_between_find_pot_and_find_pot_log2() {
        for i in 1..1000 {
            let pot = find_pot(i);
            let log = find_pot_log2(i as u32);
            assert_eq!(
                pot,
                1 << log,
                "Mismatch for input {}: pot={}, log={}",
                i,
                pot,
                log
            );
        }
    }
}
