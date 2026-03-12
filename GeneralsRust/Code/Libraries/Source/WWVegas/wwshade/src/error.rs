use thiserror::Error;

#[derive(Error, Debug)]
pub enum ShdError {
    #[error("No error")]
    NoError,

    #[error("Invalid shader configuration: {0}")]
    InvalidConfig(String),

    #[error("Hardware not supported: {0}")]
    HardwareUnsupported(String),

    #[error("Shader compilation failed: {0}")]
    CompilationFailed(String),

    #[error("Resource not found: {0}")]
    ResourceNotFound(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Serialization error: {0}")]
    Serialization(String),

    #[error("Graphics API error: {0}")]
    GraphicsApi(String),

    #[error("Load error: {0}")]
    LoadError(String),

    #[error("Format error: {0}")]
    FormatError(String),

    #[error("Out of memory")]
    OutOfMemory,

    #[error("Invalid parameter: {0}")]
    InvalidParameter(String),

    #[error("Render error: {0}")]
    RenderError(String),

    #[error("Vertex processing error: {0}")]
    VertexProcessingError(String),

    #[error("Texture error: {0}")]
    TextureError(String),
}

impl ShdError {
    /// Create a load error with a message
    pub fn load_error<S: Into<String>>(msg: S) -> Self {
        Self::LoadError(msg.into())
    }

    /// Create a format error with a message
    pub fn format_error<S: Into<String>>(msg: S) -> Self {
        Self::FormatError(msg.into())
    }

    /// Create a render error with a message
    pub fn render_error<S: Into<String>>(msg: S) -> Self {
        Self::RenderError(msg.into())
    }
}

pub type ShdResult<T> = Result<T, ShdError>;
