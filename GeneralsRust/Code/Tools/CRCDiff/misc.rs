//! Misc Module
//! 
//! Corresponds to C++ file: Tools/CRCDiff/misc.h
//! 
//! This module provides miscellaneous utility functions.

/// Convert integer to string (matches C++ intToString function)
pub fn int_to_string(val: i32) -> String {
    val.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_int_to_string() {
        assert_eq!(int_to_string(0), "0");
        assert_eq!(int_to_string(42), "42");
        assert_eq!(int_to_string(-10), "-10");
        assert_eq!(int_to_string(123456), "123456");
        assert_eq!(int_to_string(i32::MAX), i32::MAX.to_string());
        assert_eq!(int_to_string(i32::MIN), i32::MIN.to_string());
    }
}
