/*****************************************************************************
**                                                                          **
**                       Westwood Studios Pacific.                          **
**                                                                          **
**                       Confidential Information					                  **
**                Copyright (C) 2000 - All Rights Reserved                  **
**                                                                          **
******************************************************************************
**                                                                          **
** Project:  Dune Emperor                                                   **
**                                                                          **
** Module:  Audio Assert (aud_assert)                                       **
**                                                                          **
** File name:  aud_assert.rs                                                **
**                                                                          **
** Created by:  Rust conversion from audassrt.cpp                          **
**                                                                          **
** Description: Audio debugging and assertion utilities                     **
**                                                                          **
*****************************************************************************/

//! Audio assertion and debugging utilities
//! 
//! This module provides debugging and assertion functionality for the WPAudio system.
//! It includes facilities for debug printing and custom assertion handling that
//! integrate with the Windows debug output system.

use std::ffi::CString;
use std::sync::Mutex;

/// Maximum size for assertion message buffers (10KB)
const ASSERT_MSG_BUF_SIZE: usize = 10 * 1024;

/// Maximum size for general message buffers (20KB)
const MSG_BUF_SIZE: usize = ASSERT_MSG_BUF_SIZE * 2;

/// Static assertion message buffer
static ASSERT_MSG_BUF: Mutex<Vec<u8>> = Mutex::new(Vec::new());

/// Static general message buffer  
static MSG_BUF: Mutex<Vec<u8>> = Mutex::new(Vec::new());

/// Error message for invalid/dead structures
pub const DBG_TYPE_STRUCT_IS_DEAD: &str = "Invalid structure";

/// Total error count (for debugging purposes)
static TOTAL_ERRORS: Mutex<i32> = Mutex::new(0);

/// External function declaration for Windows debug printing
/// This would typically be provided by a Windows-specific module
/// For now, we'll provide a default implementation
extern "C" {
    fn WindowsDebugPrint(output_string: *const i8);
}

/// Safe wrapper around WindowsDebugPrint for Rust strings
fn windows_debug_print(message: &str) {
    // Convert Rust string to C string
    if let Ok(c_string) = CString::new(message) {
        unsafe {
            WindowsDebugPrint(c_string.as_ptr());
        }
    }
}

/// Default implementation of WindowsDebugPrint for platforms without it
#[no_mangle]
pub extern "C" fn WindowsDebugPrint(output_string: *const i8) {
    if output_string.is_null() {
        return;
    }
    
    unsafe {
        let c_str = std::ffi::CStr::from_ptr(output_string);
        if let Ok(rust_str) = c_str.to_str() {
            #[cfg(debug_assertions)]
            {
                // In debug builds, print to stderr
                eprintln!("[DEBUG] {}", rust_str);
            }
            
            #[cfg(target_os = "windows")]
            {
                // On Windows, also try to output to debugger
                use std::ffi::CString;
                if let Ok(debug_str) = CString::new(rust_str) {
                    // This would call OutputDebugStringA on Windows
                    // For now, just print to stderr as fallback
                    eprintln!("[WIN_DEBUG] {}", rust_str);
                }
            }
        }
    }
}

/// Formatted assertion printing (debug builds only)
/// 
/// This function formats a message into the assertion buffer for later use.
/// It's equivalent to the C++ `_assert_printf` function.
/// 
/// # Arguments
/// 
/// * `format_args` - Formatted arguments using Rust's format! macro system
/// 
/// # Examples
/// 
/// ```rust
/// assert_printf!("Error code: {}, message: {}", error_code, error_msg);
/// ```
#[macro_export]
macro_rules! assert_printf {
    ($($arg:tt)*) => {
        #[cfg(debug_assertions)]
        {
            $crate::aud_assert::_assert_printf_impl(format!($($arg)*))
        }
    };
}

/// Internal implementation for assert_printf macro
#[cfg(debug_assertions)]
pub fn _assert_printf_impl(message: String) {
    let mut buf = ASSERT_MSG_BUF.lock().unwrap();
    buf.clear();
    buf.extend_from_slice(message.as_bytes());
    buf.push(0); // Null terminator for C compatibility
}

/// Gets the current assertion message buffer contents
#[cfg(debug_assertions)]
pub fn get_assert_message() -> String {
    let buf = ASSERT_MSG_BUF.lock().unwrap();
    String::from_utf8_lossy(&buf[..buf.len().saturating_sub(1)]).to_string()
}

/// Audio debug printf function
/// 
/// Prints formatted debug messages to the Windows debug output.
/// This is equivalent to the C++ `_aud_debug_printf` function.
/// 
/// # Arguments
/// 
/// * `format_args` - Formatted arguments using Rust's format! macro system
/// 
/// # Examples
/// 
/// ```rust
/// aud_debug_printf!("Audio device initialized: {}", device_name);
/// ```
#[macro_export]
macro_rules! aud_debug_printf {
    ($($arg:tt)*) => {
        #[cfg(debug_assertions)]
        {
            $crate::aud_assert::_aud_debug_printf_impl(format!($($arg)*))
        }
    };
}

/// Internal implementation for aud_debug_printf macro
#[cfg(debug_assertions)]  
pub fn _aud_debug_printf_impl(message: String) {
    let mut buf = MSG_BUF.lock().unwrap();
    buf.clear();
    buf.extend_from_slice(message.as_bytes());
    buf.push(0); // Null terminator
    
    // Convert to string and send to Windows debug output
    let msg_str = String::from_utf8_lossy(&buf[..buf.len().saturating_sub(1)]);
    windows_debug_print(&msg_str);
}

/// Audio assertion function
/// 
/// Custom assertion that prints detailed error information to debug output
/// and then panics. This is equivalent to the C++ `_aud_assert` function.
/// 
/// # Arguments
/// 
/// * `_expression` - The expression that failed (unused in current implementation)
/// * `file` - Source file name where assertion failed  
/// * `line` - Line number where assertion failed
/// * `reason` - Description of why the assertion failed
/// 
/// # Panics
/// 
/// Always panics after logging the assertion failure
#[cfg(debug_assertions)]
pub fn aud_assert(_expression: &str, file: &str, line: u32, reason: &str) {
    let mut buf = MSG_BUF.lock().unwrap();
    buf.clear();
    
    let error_msg = format!("{}({}) : Error : ASSERT - {}\n", file, line, reason);
    buf.extend_from_slice(error_msg.as_bytes());
    buf.push(0);
    
    // Send to Windows debug output
    let msg_str = String::from_utf8_lossy(&buf[..buf.len().saturating_sub(1)]);
    windows_debug_print(&msg_str);
    
    // Increment error counter
    {
        let mut errors = TOTAL_ERRORS.lock().unwrap();
        *errors += 1;
    }
    
    // In Rust, we panic instead of calling the C assert
    panic!("Audio assertion failed: {} at {}:{}", reason, file, line);
}

/// Macro for audio assertions that automatically captures file and line information
/// 
/// This macro provides a convenient way to perform assertions with automatic
/// file and line number capture, similar to the standard assert! macro but
/// specifically for audio debugging.
/// 
/// # Arguments
/// 
/// * `condition` - Boolean expression to test
/// * `message` - Optional message to display if assertion fails
/// 
/// # Examples
/// 
/// ```rust
/// aud_assert_check!(buffer.len() > 0, "Audio buffer cannot be empty");
/// aud_assert_check!(sample_rate > 0);
/// ```
#[macro_export]
macro_rules! aud_assert_check {
    ($condition:expr, $message:expr) => {
        #[cfg(debug_assertions)]
        {
            if !($condition) {
                $crate::aud_assert::aud_assert(
                    stringify!($condition),
                    file!(),
                    line!(),
                    $message
                );
            }
        }
    };
    ($condition:expr) => {
        #[cfg(debug_assertions)]
        {
            if !($condition) {
                $crate::aud_assert::aud_assert(
                    stringify!($condition),
                    file!(),
                    line!(),
                    &format!("Assertion failed: {}", stringify!($condition))
                );
            }
        }
    };
}

/// Gets the current total error count
/// 
/// Returns the number of assertion failures that have occurred.
/// Useful for debugging and testing purposes.
/// 
/// # Returns
/// 
/// The total number of errors/assertions that have been triggered
pub fn get_total_errors() -> i32 {
    let errors = TOTAL_ERRORS.lock().unwrap();
    *errors
}

/// Resets the total error count to zero
/// 
/// Useful for testing scenarios where you want to start with a clean slate.
pub fn reset_error_count() {
    let mut errors = TOTAL_ERRORS.lock().unwrap();
    *errors = 0;
}

/// Result type for operations that might fail with audio-related errors
/// 
/// This provides a more idiomatic Rust way of handling errors compared to
/// the original C++ assertion-based error handling.
pub type AudioResult<T> = Result<T, AudioError>;

/// Audio-specific error types
/// 
/// Provides structured error handling as an alternative to assertions
/// for recoverable error conditions.
#[derive(Debug, Clone)]
pub enum AudioError {
    /// Invalid parameter provided to function
    InvalidParameter(String),
    /// Audio device not available or not found
    DeviceNotFound(String),
    /// Buffer operation failed
    BufferError(String),
    /// Audio format not supported
    UnsupportedFormat(String),
    /// General audio system error
    AudioSystemError(String),
    /// Invalid structure/object state
    InvalidStructure(String),
}

impl std::fmt::Display for AudioError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AudioError::InvalidParameter(msg) => write!(f, "Invalid parameter: {}", msg),
            AudioError::DeviceNotFound(msg) => write!(f, "Device not found: {}", msg),
            AudioError::BufferError(msg) => write!(f, "Buffer error: {}", msg),
            AudioError::UnsupportedFormat(msg) => write!(f, "Unsupported format: {}", msg),
            AudioError::AudioSystemError(msg) => write!(f, "Audio system error: {}", msg),
            AudioError::InvalidStructure(msg) => write!(f, "Invalid structure: {}", msg),
        }
    }
}

impl std::error::Error for AudioError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_count_operations() {
        reset_error_count();
        assert_eq!(get_total_errors(), 0);
    }

    #[test]
    fn test_debug_message_formatting() {
        #[cfg(debug_assertions)]
        {
            assert_printf!("Test message: {}", 42);
            let msg = get_assert_message();
            assert_eq!(msg, "Test message: 42");
        }
    }

    #[test]
    fn test_audio_error_display() {
        let error = AudioError::InvalidParameter("test parameter".to_string());
        assert_eq!(error.to_string(), "Invalid parameter: test parameter");
    }

    #[test] 
    #[should_panic(expected = "Audio assertion failed")]
    #[cfg(debug_assertions)]
    fn test_audio_assertion_panic() {
        aud_assert("false", "test.rs", 123, "Test assertion failure");
    }
}