//! Option extension methods
//!
//! Provides additional utility methods for Option types to match
//! C++ style object management patterns.

use std::sync::{Arc, Mutex, Weak};

/// Extension trait for Option to provide upgrade functionality (mirrors C++ weak_ptr::lock).
pub trait OptionExt<T> {
    /// Upgrade a weak reference (or return the existing strong reference).
    fn upgrade(&self) -> Option<Arc<Mutex<T>>>;
}

impl<T> OptionExt<T> for Option<Weak<Mutex<T>>> {
    fn upgrade(&self) -> Option<Arc<Mutex<T>>> {
        self.as_ref()?.upgrade()
    }
}

impl<T> OptionExt<T> for Option<Arc<Mutex<T>>> {
    fn upgrade(&self) -> Option<Arc<Mutex<T>>> {
        self.clone()
    }
}

/// Extension trait for Result to provide ok functionality
pub trait ResultExt<T, E> {
    /// Convert Result to Option, discarding error
    fn ok(self) -> Option<T>;
}

impl<T, E> ResultExt<T, E> for Result<T, E> {
    fn ok(self) -> Option<T> {
        match self {
            Ok(val) => Some(val),
            Err(_) => None,
        }
    }
}
