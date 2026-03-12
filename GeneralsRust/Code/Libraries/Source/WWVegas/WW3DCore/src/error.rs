use thiserror::Error;

pub type Result<T, E = W3dError> = std::result::Result<T, E>;

#[derive(Debug, Error)]
pub enum W3dError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Invalid data: {0}")]
    InvalidData(String),

    #[error("Unsupported: {0}")]
    Unsupported(String),

    #[error("Parse error: {0}")]
    Parse(String),
}
