//! Error types for the download crate

use thiserror::Error;

/// Download operation errors
#[derive(Error, Debug, Clone)]
pub enum DownloadError {
    #[error("Invalid parameters: {0}")]
    InvalidParameters(String),

    #[error("Network connection failed: {0}")]
    NetworkError(String),

    #[error("FTP operation failed: {0}")]
    FtpError(String),

    #[error("HTTP operation failed: {0}")]
    HttpError(String),

    #[error("File system error: {0}")]
    FileSystemError(String),

    #[error("Authentication failed: {0}")]
    AuthenticationError(String),

    #[error("File not found: {0}")]
    FileNotFound(String),

    #[error("Download was cancelled")]
    Cancelled,

    #[error("Configuration error: {0}")]
    ConfigError(String),

    #[error("URL parsing error: {0}")]
    UrlParseError(String),

    #[error("Timeout occurred during operation")]
    Timeout,

    #[error("Resume operation failed: {0}")]
    ResumeError(String),
}

/// Download status codes
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DownloadStatus {
    None = 0,
    Connecting = 1,
    Authenticating = 2,
    FindingFile = 3,
    Downloading = 4,
    Finished = 5,
    Error = 6,
}

/// Event types for download callbacks
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DownloadEvent {
    Started,
    Progress,
    StatusChange,
    QueryResume,
    Finished,
    Error,
}

/// Result type for download operations
pub type DownloadResult<T> = Result<T, DownloadError>;

impl From<suppaftp::FtpError> for DownloadError {
    fn from(err: suppaftp::FtpError) -> Self {
        DownloadError::FtpError(err.to_string())
    }
}

impl From<reqwest::Error> for DownloadError {
    fn from(err: reqwest::Error) -> Self {
        DownloadError::HttpError(err.to_string())
    }
}

impl From<std::io::Error> for DownloadError {
    fn from(err: std::io::Error) -> Self {
        DownloadError::FileSystemError(err.to_string())
    }
}

impl From<url::ParseError> for DownloadError {
    fn from(err: url::ParseError) -> Self {
        DownloadError::UrlParseError(err.to_string())
    }
}
