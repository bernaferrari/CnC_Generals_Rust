/// WW3D Error types
/// Ported from w3derr.h
#[derive(Debug, thiserror::Error, PartialEq)]
pub enum W3DError {
    #[error("Invalid chunk type encountered: {0}")]
    InvalidChunkType(u32),

    #[error("Renderer is not currently registered")]
    RendererUnavailable,

    #[error("Feature disabled: {0}")]
    FeatureDisabled(String),

    #[error("Unsupported type: {0}")]
    UnsupportedType(String),

    #[error("File format version not supported")]
    UnsupportedVersion,

    #[error("Corrupted or invalid W3D file")]
    CorruptedFile,

    #[error("Asset not found: {0}")]
    AssetNotFound(String),

    #[error("Out of memory")]
    OutOfMemory,

    #[error("Invalid parameter: {0}")]
    InvalidParameter(String),

    #[error("IO error: {0}")]
    IoError(String),

    #[error("Not initialized: {0}")]
    NotInitialized(String),

    #[error("Render error: {0}")]
    RenderError(String),

    #[error("Unknown error")]
    Unknown,

    #[error("Unknown error: {0}")]
    UnknownWithMessage(String),
}

pub type W3DResult<T> = Result<T, W3DError>;

impl From<std::io::Error> for W3DError {
    fn from(err: std::io::Error) -> Self {
        W3DError::IoError(err.to_string())
    }
}

impl From<binrw::Error> for W3DError {
    fn from(err: binrw::Error) -> Self {
        W3DError::IoError(err.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_display() {
        let err = W3DError::AssetNotFound("tank.w3d".to_string());
        assert_eq!(err.to_string(), "Asset not found: tank.w3d");
    }
}
