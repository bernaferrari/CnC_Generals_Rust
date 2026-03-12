//! Result extension methods
//!
//! Provides additional utility methods for Result types

/// Extension trait for Result to provide additional functionality.
/// The C++ code occasionally pushes boolean arguments into error stacks; in Rust
/// we keep this as a no-op for parity with call sites that expect the method to
/// exist without altering the result.
pub trait ResultExt<T, E> {
    /// Append a boolean argument to the result (no-op for parity).
    fn append_boolean_argument(self, _arg: bool) -> Self;
}

impl<T, E> ResultExt<T, E> for Result<T, E> {
    fn append_boolean_argument(self, _arg: bool) -> Self {
        // Left intentionally as a no-op: we don't carry extra boolean context in Result.
        self
    }
}
