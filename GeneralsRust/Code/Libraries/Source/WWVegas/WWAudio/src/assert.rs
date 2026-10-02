//! Audio-specific assertion and debugging utilities.

/// Audio assertion macros and utilities
///
/// This module provides WPAudio-specific assertion macros that can be
/// conditionally compiled and provide detailed audio system state information.

/// Audio assertion result
#[derive(Debug)]
pub enum AssertResult {
    Success,
    Failed(String),
}

use parking_lot::Mutex;

/// Audio assertion handler trait
pub trait AssertHandler {
    fn handle_assertion(&self, message: &str, file: &str, line: u32);
}

/// Default assertion handler that logs to console
pub struct DefaultAssertHandler;

impl AssertHandler for DefaultAssertHandler {
    fn handle_assertion(&self, message: &str, file: &str, line: u32) {
        eprintln!("[WPAudio ASSERT] {}:{} - {}", file, line, message);

        #[cfg(debug_assertions)]
        {
            panic!("Audio assertion failed: {}", message);
        }
    }
}

/// Global assertion handler store (`None` means `DefaultAssertHandler`).
// THREAD: asserts can fire from any thread (game thread and the audio
// callback thread), while the handler can be swapped from the game thread,
// so this process-global slot stays a mutex.
static ASSERT_HANDLER: Mutex<Option<Box<dyn AssertHandler + Send + Sync>>> = Mutex::new(None);

/// Set custom assertion handler
pub fn set_assert_handler(handler: Box<dyn AssertHandler + Send + Sync>) {
    *ASSERT_HANDLER.lock() = Some(handler);
}

/// Internal assertion function
#[doc(hidden)]
pub fn assert_internal(condition: bool, message: &str, file: &str, line: u32) {
    if !condition {
        let guard = ASSERT_HANDLER.lock();
        match guard.as_deref() {
            Some(handler) => handler.handle_assertion(message, file, line),
            None => DefaultAssertHandler.handle_assertion(message, file, line),
        }
    }
}

/// Audio-specific assertion macro
#[macro_export]
macro_rules! audio_assert {
    ($condition:expr) => {
        $crate::assert::assert_internal($condition, stringify!($condition), file!(), line!())
    };
    ($condition:expr, $message:literal) => {
        $crate::assert::assert_internal($condition, $message, file!(), line!())
    };
    ($condition:expr, $format:literal, $($args:expr),*) => {
        $crate::assert::assert_internal(
            $condition,
            &format!($format, $($args),*),
            file!(),
            line!()
        )
    };
}

/// Debug-only assertion macro
#[macro_export]
macro_rules! audio_debug_assert {
    ($($args:tt)*) => {
        #[cfg(debug_assertions)]
        $crate::audio_assert!($($args)*);
    };
}

/// Assert that a value is within valid audio range
#[macro_export]
macro_rules! audio_assert_range {
    ($value:expr, $min:expr, $max:expr) => {
        $crate::audio_assert!(
            $value >= $min && $value <= $max,
            "Value {} not in range [{}, {}]",
            $value,
            $min,
            $max
        )
    };
}

/// Assert that audio format is valid
#[macro_export]
macro_rules! audio_assert_format {
    ($format:expr) => {
        $crate::audio_assert!(
            $format.is_supported(),
            "Invalid audio format: {:?}",
            $format
        )
    };
}

/// Assert that volume is in valid range
#[macro_export]
macro_rules! audio_assert_volume {
    ($volume:expr) => {
        $crate::audio_assert_range!($volume, $crate::MIN_VOLUME, $crate::MAX_VOLUME)
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_assertion_macros() {
        // These should not panic in release mode
        audio_assert!(true);
        audio_assert!(true, "This should pass");

        // Test range assertion
        audio_assert_range!(50, 0, 100);
    }

    #[test]
    #[should_panic]
    #[cfg(debug_assertions)]
    fn test_failing_assertion() {
        audio_assert!(false, "This should fail");
    }
}
