//! Debug Module
//! 
//! Corresponds to C++ file: Tools/CRCDiff/debug.cpp
//! 
//! This module provides minimal debug information functionality.

use std::io::{self, Write};

/// Debug logging function (matches C++ DebugLog)
pub fn debug_log(message: &str) {
    #[cfg(debug_assertions)]
    {
        print!("{}", message);
        // In Windows, this would also call OutputDebugString
        // For cross-platform compatibility, we just use stdout
        let _ = io::stdout().flush();
    }
    #[cfg(not(debug_assertions))]
    {
        // In release builds, do nothing (matches #ifndef DEBUG behavior)
        let _ = message;
    }
}

/// Debug logging macro (matches C++ DEBUG_LOG macro)
#[macro_export]
macro_rules! debug_log {
    ($($args:tt)*) => {
        #[cfg(debug_assertions)]
        {
            let message = format!($($args)*);
            $crate::debug::debug_log(&message);
        }
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_debug_log() {
        // Test basic debug logging
        debug_log("Test message\n");
        
        // Test with debug_log! macro
        debug_log!("Test formatted message: {}\n", 42);
        
        assert!(true); // This test mainly verifies compilation
    }
    
    #[test]
    fn test_debug_log_empty() {
        debug_log("");
        debug_log!("");
        assert!(true);
    }
    
    #[test] 
    fn test_debug_log_formatting() {
        debug_log!("Number: {}, String: {}, Bool: {}\n", 123, "test", true);
        assert!(true);
    }
}
