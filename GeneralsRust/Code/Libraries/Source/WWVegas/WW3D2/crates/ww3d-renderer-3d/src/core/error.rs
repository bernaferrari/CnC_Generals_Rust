//! Core error types for WW3D renderer

use thiserror::Error as ThisError;

// Re-export ww3d_core errors for compatibility
pub use ww3d_core::errors::W3DError;

/// Result type for WW3D operations
pub type Result<T> = std::result::Result<T, Error>;
pub type RendererResult<T> = Result<T>;

/// Error type for WW3D operations (matching original C++ WW3DErrorType)
#[derive(Debug, ThisError, Clone, PartialEq, Eq)]
pub enum Error {
    /// Operation succeeded (kept for parity with legacy error codes)
    #[error("operation succeeded")]
    Ok,
    /// Generic error with message
    #[error("generic error: {0}")]
    Generic(String),
    /// Alternate naming used throughout the renderer
    #[error("generic error: {0}")]
    GenericError(String),
    /// Load operation failed
    #[error("load operation failed")]
    LoadFailed,
    /// Save operation failed
    #[error("save operation failed")]
    SaveFailed,
    /// Window not open
    #[error("window not open")]
    WindowNotOpen,
    /// Initialization failed without additional context
    #[error("initialization failed")]
    InitializationFailed,
    /// Invalid parameter supplied to an API
    #[error("invalid parameter: {0}")]
    InvalidParameter(String),
    /// Invalid operation invoked on the current resource state
    #[error("invalid operation: {0}")]
    InvalidOperation(String),
    /// Resource requested was not initialized yet
    #[error("not initialized: {0}")]
    NotInitialized(String),
    /// Feature or code path not implemented yet
    #[error("not implemented: {0}")]
    NotImplemented(String),
    /// Underlying GPU device has not been initialized
    #[error("device not initialized: {0}")]
    DeviceNotInitialized(String),
    /// Required GPU adapter could not be located
    #[error("adapter not found: {0}")]
    AdapterNotFound(String),
    /// Requested resource could not be located
    #[error("resource not found: {0}")]
    ResourceNotFound(String),
    /// Out-of-memory condition while fulfilling a request
    #[error("out of memory: {0}")]
    OutOfMemory(String),
    /// Render specific failure with context
    #[error("render error: {0}")]
    RenderError(String),
    /// Buffer growth exceeded the permitted capacity
    #[error("buffer overflow: {0}")]
    BufferOverflow(String),
    /// Invalid mip level requested
    #[error("invalid mip level: {0}")]
    InvalidMipLevel(String),
    /// Invalid texture payload data encountered
    #[error("invalid texture data: {0}")]
    InvalidTextureData(String),
    /// Invalid vertex format description supplied
    #[error("invalid vertex format: {0}")]
    InvalidVertexFormat(String),
    /// Unsupported vertex format requested
    #[error("unsupported vertex format: {0}")]
    UnsupportedVertexFormat(String),
    /// Platform specific functionality not supported
    #[error("platform not supported: {0}")]
    PlatformNotSupported(String),
    /// Invalid data encountered
    #[error("invalid data: {0}")]
    InvalidData(String),
    /// File not found
    #[error("file not found: {0}")]
    FileNotFound(String),
}

/// Alias for compatibility with original C++ naming
pub type WW3DErrorType = Error;

/// Constants matching original C++ WW3DErrorType values
pub const WW3D_ERROR_OK: Error = Error::Ok;
pub const WW3D_ERROR_LOAD_FAILED: Error = Error::LoadFailed;
pub const WW3D_ERROR_SAVE_FAILED: Error = Error::SaveFailed;
pub const WW3D_ERROR_WINDOW_NOT_OPEN: Error = Error::WindowNotOpen;
pub const WW3D_ERROR_INITIALIZATION_FAILED: Error = Error::InitializationFailed;

impl From<wgpu::Error> for Error {
    fn from(err: wgpu::Error) -> Self {
        Error::Generic(err.to_string())
    }
}

impl From<wgpu::SurfaceError> for Error {
    fn from(err: wgpu::SurfaceError) -> Self {
        Error::Generic(err.to_string())
    }
}

impl From<std::io::Error> for Error {
    fn from(err: std::io::Error) -> Self {
        Error::Generic(err.to_string())
    }
}

impl From<W3DError> for Error {
    fn from(err: W3DError) -> Self {
        match err {
            W3DError::InvalidChunkType(chunk_type) => {
                Error::InvalidData(format!("Invalid chunk type: 0x{:X}", chunk_type))
            }
            W3DError::UnsupportedVersion => Error::InvalidData("Unsupported version".to_string()),
            W3DError::CorruptedFile => Error::InvalidData("Corrupted file".to_string()),
            W3DError::AssetNotFound(msg) => Error::ResourceNotFound(msg),
            W3DError::OutOfMemory => Error::OutOfMemory("Out of memory".to_string()),
            W3DError::InvalidParameter(msg) => Error::InvalidParameter(msg),
            W3DError::IoError(msg) => Error::Generic(msg),
            W3DError::NotInitialized(msg) => Error::NotInitialized(msg),
            W3DError::RendererUnavailable => {
                Error::InvalidOperation("renderer unavailable".to_string())
            }
            W3DError::FeatureDisabled(msg) => Error::InvalidOperation(msg),
            W3DError::UnsupportedType(msg) => Error::InvalidOperation(msg),
            W3DError::RenderError(msg) => Error::RenderError(msg),
            W3DError::Unknown => Error::Generic("Unknown error".to_string()),
            W3DError::UnknownWithMessage(msg) => Error::Generic(msg),
        }
    }
}
impl From<Error> for W3DError {
    fn from(err: Error) -> Self {
        match err {
            Error::InvalidParameter(msg) => W3DError::InvalidParameter(msg),
            Error::ResourceNotFound(msg) | Error::FileNotFound(msg) => W3DError::AssetNotFound(msg),
            Error::OutOfMemory(_) => W3DError::OutOfMemory,
            Error::NotInitialized(msg) | Error::DeviceNotInitialized(msg) => {
                W3DError::NotInitialized(msg)
            }
            Error::RenderError(msg) => W3DError::RenderError(msg),
            Error::Generic(msg) | Error::GenericError(msg) => W3DError::IoError(msg),
            Error::Ok => W3DError::UnknownWithMessage("operation succeeded".to_string()),
            Error::LoadFailed => W3DError::UnknownWithMessage("load operation failed".to_string()),
            Error::SaveFailed => W3DError::UnknownWithMessage("save operation failed".to_string()),
            Error::WindowNotOpen => W3DError::NotInitialized("window not open".to_string()),
            Error::InitializationFailed => {
                W3DError::NotInitialized("renderer initialization failed".to_string())
            }
            Error::InvalidOperation(msg) => {
                W3DError::UnknownWithMessage(format!("invalid operation: {msg}"))
            }
            Error::NotImplemented(msg) => {
                W3DError::UnknownWithMessage(format!("not implemented: {msg}"))
            }
            Error::AdapterNotFound(msg) => {
                W3DError::UnknownWithMessage(format!("adapter not found: {msg}"))
            }
            Error::BufferOverflow(msg) => {
                W3DError::UnknownWithMessage(format!("buffer overflow: {msg}"))
            }
            Error::InvalidMipLevel(msg) => {
                W3DError::UnknownWithMessage(format!("invalid mip level: {msg}"))
            }
            Error::InvalidTextureData(msg) => {
                W3DError::UnknownWithMessage(format!("invalid texture data: {msg}"))
            }
            Error::InvalidVertexFormat(msg) => {
                W3DError::UnknownWithMessage(format!("invalid vertex format: {msg}"))
            }
            Error::UnsupportedVertexFormat(msg) => {
                W3DError::UnknownWithMessage(format!("unsupported vertex format: {msg}"))
            }
            Error::PlatformNotSupported(msg) => {
                W3DError::UnknownWithMessage(format!("platform not supported: {msg}"))
            }
            Error::InvalidData(msg) => W3DError::UnknownWithMessage(format!("invalid data: {msg}")),
        }
    }
}

impl Error {
    /// Map error to legacy numeric code for compatibility with diagnostics that mirror the C++ layer.
    pub fn code(&self) -> i32 {
        match self {
            Error::Ok => 0,
            Error::Generic(_) | Error::GenericError(_) => -1,
            Error::LoadFailed => -2,
            Error::SaveFailed => -3,
            Error::WindowNotOpen => -4,
            Error::InitializationFailed => -5,
            Error::InvalidParameter(_) => -6,
            Error::InvalidOperation(_) => -7,
            Error::NotInitialized(_) => -8,
            Error::NotImplemented(_) => -9,
            Error::DeviceNotInitialized(_) => -10,
            Error::AdapterNotFound(_) => -11,
            Error::ResourceNotFound(_) => -12,
            Error::OutOfMemory(_) => -13,
            Error::RenderError(_) => -14,
            Error::BufferOverflow(_) => -15,
            Error::InvalidMipLevel(_) => -16,
            Error::InvalidTextureData(_) => -17,
            Error::InvalidVertexFormat(_) => -18,
            Error::UnsupportedVertexFormat(_) => -19,
            Error::PlatformNotSupported(_) => -20,
            Error::InvalidData(_) => -21,
            Error::FileNotFound(_) => -22,
        }
    }
}
