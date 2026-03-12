// FILE: errors.rs
// Ported from C++ Errors.h

/// Error codes used throughout the engine.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum ErrorCode {
    /// Base value (distinctive)
    ErrorBase = 0xdead0001,
    /// Should not be possible under normal operation
    ErrorBug = 0xdead0002,
    /// Unable to allocate memory
    ErrorOutOfMemory = 0xdead0003,
    /// Generic bad argument
    ErrorBadArg = 0xdead0004,
    /// Unrecognized file version
    ErrorInvalidFileVersion = 0xdead0005,
    /// Invalid file format
    ErrorCorruptFileFormat = 0xdead0006,
    /// Bad INI data
    ErrorBadIni = 0xdead0007,
    /// Error initializing Direct3D
    ErrorInvalidD3D = 0xdead0008,
    /// Sentinel end value
    ErrorLast = 0xdead0009,
}
