//! Modern Rust download library for Command & Conquer Generals Zero Hour
//!
//! This crate provides async download functionality supporting both FTP and HTTP protocols
//! with features like resume support, progress tracking, and cross-platform configuration.
//!
//! # Features
//! - **Async/await support**: Built on tokio for efficient concurrent operations
//! - **FTP and HTTP downloads**: Support for both protocols with unified interface
//! - **Resume support**: Automatic resume of interrupted downloads
//! - **Progress tracking**: Real-time progress callbacks with rate calculation
//! - **Cross-platform config**: JSON-based configuration replacing Windows registry
//! - **Error handling**: Comprehensive error types with detailed error information
//! - **Retry logic**: Automatic retry with exponential backoff for network failures
//! - **Modern security**: TLS support for secure connections
//!
//! # Examples
//!
//! ## Simple HTTP download
//! ```rust,no_run
//! use download::{DownloadManager, ConsoleDownloadListener};
//! use std::sync::Arc;
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     let mut manager = DownloadManager::new()?;
//!     let listener = Arc::new(ConsoleDownloadListener);
//!     
//!     let bytes_downloaded = manager.download_http(
//!         "https://example.com/file.zip",
//!         "/tmp/downloaded_file.zip",
//!         Some(listener),
//!     ).await?;
//!     
//!     println!("Downloaded {} bytes", bytes_downloaded);
//!     Ok(())
//! }
//! ```
//!
//! ## FTP download with custom callback
//! ```rust,no_run
//! use download::{DownloadManager, DownloadListener, ProgressInfo, DownloadStatus, DownloadError};
//! use async_trait::async_trait;
//! use std::sync::Arc;
//!
//! struct MyListener;
//!
//! #[async_trait]
//! impl DownloadListener for MyListener {
//!     async fn on_progress(&self, progress: ProgressInfo) {
//!         if let Some(ratio) = progress.completion_ratio() {
//!             println!("Progress: {:.1}%", ratio * 100.0);
//!         }
//!     }
//!     
//!     async fn on_completed(&self) {
//!         println!("Download finished!");
//!     }
//!     
//!     async fn on_error(&self, error: DownloadError) {
//!         eprintln!("Error: {}", error);
//!     }
//!     
//!     async fn should_resume(&self) -> bool {
//!         true
//!     }
//!     
//!     async fn on_status_change(&self, status: DownloadStatus) {
//!         println!("Status: {:?}", status);
//!     }
//! }
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     let mut manager = DownloadManager::new()?;
//!     let listener = Arc::new(MyListener);
//!     
//!     let bytes_downloaded = manager.download_ftp(
//!         "ftp.example.com",
//!         "anonymous",
//!         "guest@example.com",
//!         "/pub/file.zip",
//!         "/tmp/file.zip",
//!         Some(listener),
//!     ).await?;
//!     
//!     println!("Downloaded {} bytes", bytes_downloaded);
//!     Ok(())
//! }
//! ```

// Re-export main types for convenience
pub mod download_debug;
pub mod downloaddefs;
pub mod ftp;
pub mod ftpdefs;
pub mod registry;
pub use config::{ConfigManager, DownloadConfig};
pub use download::{ConsoleDownloadListener, DownloadListener, DownloadManager, ProgressInfo};
pub use error::{DownloadError, DownloadEvent, DownloadResult, DownloadStatus};
pub use ftp_client::{FtpClient, FtpConfig, FtpProgressCallback};
pub use registry::{
    get_string_from_registry, get_unsigned_int_from_registry, set_string_in_registry,
    set_unsigned_int_in_registry,
};
pub use url_builder::{format_urls_from_config, UrlBuilder, UrlConfig};

// Internal modules
mod config;
mod download;
mod error;
mod ftp_client;
mod url_builder;

// Compatibility layer for C++ code migration
pub mod compat {
    //! Compatibility functions to ease migration from C++ code
    //!
    //! These functions provide a similar interface to the original C++ implementation
    //! but use modern async patterns internally.

    use crate::{ConsoleDownloadListener, DownloadManager, DownloadResult};
    use std::sync::Arc;

    /// Create a download manager (equivalent to CDownload constructor)
    pub fn create_download_manager() -> DownloadResult<DownloadManager> {
        DownloadManager::new()
    }

    /// Simple FTP download function (similar to original DownloadFile method)
    pub async fn download_file_ftp(
        server: &str,
        username: &str,
        password: &str,
        remote_file: &str,
        local_file: &str,
    ) -> DownloadResult<u64> {
        let mut manager = DownloadManager::new()?;
        let listener = Arc::new(ConsoleDownloadListener);

        manager
            .download_ftp(
                server,
                username,
                password,
                remote_file,
                local_file,
                Some(listener),
            )
            .await
    }

    /// Simple HTTP download function
    pub async fn download_file_http(url: &str, local_file: &str) -> DownloadResult<u64> {
        let mut manager = DownloadManager::new()?;
        let listener = Arc::new(ConsoleDownloadListener);

        manager.download_http(url, local_file, Some(listener)).await
    }
}

// Version information
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Initialize the download library
///
/// This should be called once at the beginning of the application to set up
/// logging and other global resources.
pub fn init() {
    // Initialize logging if not already done
    if std::env::var("RUST_LOG").is_err() {
        std::env::set_var("RUST_LOG", "download=info");
    }

    // Initialize tracing subscriber if none exists
    let _ = tracing_subscriber::fmt::try_init();
}
