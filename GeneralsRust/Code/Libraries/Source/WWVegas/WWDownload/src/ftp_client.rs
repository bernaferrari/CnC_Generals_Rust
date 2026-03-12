//! Modern async FTP client implementation
//!
//! Replaces the original synchronous Windows socket-based FTP implementation
//! with a modern async Rust implementation using the suppaftp crate.

use crate::error::{DownloadError, DownloadResult, DownloadStatus};
use async_trait::async_trait;
use futures::AsyncReadExt;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};
use suppaftp::AsyncFtpStream;
use tokio::fs::{File, OpenOptions};
use tokio::io::{AsyncSeekExt, AsyncWriteExt, SeekFrom};
use tokio::time::timeout;
use tracing::{debug, info, warn};

const DEFAULT_TIMEOUT: Duration = Duration::from_secs(30);
const CHUNK_SIZE: usize = 8192;
const MAX_RETRIES: usize = 3;

/// Progress callback trait for FTP operations
#[async_trait]
pub trait FtpProgressCallback: Send + Sync {
    async fn on_progress(&self, bytes_read: u64, total_size: Option<u64>, elapsed: Duration);
    async fn on_status_change(&self, status: DownloadStatus);
    async fn on_error(&self, error: &DownloadError);
    async fn query_resume(&self) -> bool;
}

/// FTP client configuration
#[derive(Debug, Clone)]
pub struct FtpConfig {
    pub server: String,
    pub port: u16,
    pub username: String,
    pub password: String,
    pub passive_mode: bool,
    pub timeout: Duration,
    pub retry_count: usize,
}

impl Default for FtpConfig {
    fn default() -> Self {
        Self {
            server: String::new(),
            port: 21,
            username: "anonymous".to_string(),
            password: "guest@example.com".to_string(),
            passive_mode: true,
            timeout: DEFAULT_TIMEOUT,
            retry_count: MAX_RETRIES,
        }
    }
}

/// Modern async FTP client
pub struct FtpClient {
    config: FtpConfig,
    stream: Option<AsyncFtpStream>,
    current_dir: String,
    connected: bool,
}

impl FtpClient {
    /// Create a new FTP client with configuration
    pub fn new(config: FtpConfig) -> Self {
        Self {
            config,
            stream: None,
            current_dir: "/".to_string(),
            connected: false,
        }
    }

    /// Connect to the FTP server
    pub async fn connect(&mut self) -> DownloadResult<()> {
        info!(
            "Connecting to FTP server {}:{}",
            self.config.server, self.config.port
        );

        let connect_future =
            AsyncFtpStream::connect(format!("{}:{}", self.config.server, self.config.port));

        let stream = timeout(self.config.timeout, connect_future)
            .await
            .map_err(|_| DownloadError::Timeout)?
            .map_err(|e| DownloadError::FtpError(format!("Failed to connect: {}", e)))?;

        self.stream = Some(stream);
        self.connected = true;

        debug!("Connected to FTP server");
        Ok(())
    }

    /// Login to the FTP server
    pub async fn login(&mut self) -> DownloadResult<()> {
        let stream = self
            .stream
            .as_mut()
            .ok_or_else(|| DownloadError::NetworkError("Not connected".to_string()))?;

        info!("Logging in as user: {}", self.config.username);

        let login_future = stream.login(&self.config.username, &self.config.password);

        timeout(self.config.timeout, login_future)
            .await
            .map_err(|_| DownloadError::Timeout)?
            .map_err(|e| DownloadError::AuthenticationError(format!("Login failed: {}", e)))?;

        debug!("Successfully logged in");
        Ok(())
    }

    /// Change to a directory
    pub async fn change_directory(&mut self, path: &str) -> DownloadResult<()> {
        let stream = self
            .stream
            .as_mut()
            .ok_or_else(|| DownloadError::NetworkError("Not connected".to_string()))?;

        debug!("Changing directory to: {}", path);

        let cwd_future = stream.cwd(path);

        timeout(self.config.timeout, cwd_future)
            .await
            .map_err(|_| DownloadError::Timeout)?
            .map_err(|e| DownloadError::FtpError(format!("Failed to change directory: {}", e)))?;

        self.current_dir = path.to_string();
        Ok(())
    }

    /// Get file size
    pub async fn get_file_size(&mut self, remote_path: &str) -> DownloadResult<u64> {
        let stream = self
            .stream
            .as_mut()
            .ok_or_else(|| DownloadError::NetworkError("Not connected".to_string()))?;

        debug!("Getting file size for: {}", remote_path);

        let size_future = stream.size(remote_path);

        let size = timeout(self.config.timeout, size_future)
            .await
            .map_err(|_| DownloadError::Timeout)?
            .map_err(|e| {
                DownloadError::FileNotFound(format!("File not found or size unavailable: {}", e))
            })?;

        Ok(size as u64)
    }

    /// Download a file with progress callback and resume support
    pub async fn download_file<P, C>(
        &mut self,
        remote_path: &str,
        local_path: P,
        callback: Option<&C>,
    ) -> DownloadResult<u64>
    where
        P: AsRef<Path>,
        C: FtpProgressCallback + ?Sized,
    {
        let local_path = local_path.as_ref();

        info!(
            "Starting download: {} -> {}",
            remote_path,
            local_path.display()
        );

        // Get file size
        let total_size = match self.get_file_size(remote_path).await {
            Ok(size) => {
                debug!("Remote file size: {} bytes", size);
                Some(size)
            }
            Err(e) => {
                warn!("Could not get file size: {}", e);
                None
            }
        };

        // Check for resume
        let (resume_pos, file) = self
            .prepare_local_file(local_path, total_size, callback)
            .await?;

        if let Some(callback) = callback {
            callback.on_status_change(DownloadStatus::Downloading).await;
        }

        let bytes_downloaded = self
            .download_with_resume(remote_path, file, resume_pos, total_size, callback)
            .await?;

        info!("Download completed: {} bytes", bytes_downloaded);
        Ok(bytes_downloaded)
    }

    /// Prepare local file for download, handling resume
    async fn prepare_local_file<P, C>(
        &self,
        local_path: P,
        total_size: Option<u64>,
        callback: Option<&C>,
    ) -> DownloadResult<(u64, File)>
    where
        P: AsRef<Path>,
        C: FtpProgressCallback + ?Sized,
    {
        let local_path = local_path.as_ref();

        // Create parent directories if needed
        if let Some(parent) = local_path.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }

        // Check if file exists and can be resumed
        let resume_pos = if local_path.exists() {
            let metadata = tokio::fs::metadata(local_path).await?;
            let current_size = metadata.len();

            if let Some(total) = total_size {
                if current_size == total {
                    info!("File already complete, skipping download");
                    return Ok((total, File::create(local_path).await?));
                } else if current_size > 0 && current_size < total {
                    // Ask callback if we should resume
                    let should_resume = if let Some(callback) = callback {
                        callback.query_resume().await
                    } else {
                        true // Default to resuming
                    };

                    if should_resume {
                        info!("Resuming download from byte {}", current_size);
                        current_size
                    } else {
                        info!("Not resuming, starting fresh download");
                        0
                    }
                } else {
                    warn!("Local file is larger than remote, starting fresh");
                    0
                }
            } else {
                // Unknown remote size, assume resume
                current_size
            }
        } else {
            0
        };

        let file = if resume_pos > 0 {
            let mut file = OpenOptions::new()
                .write(true)
                .append(true)
                .open(local_path)
                .await?;
            file.seek(SeekFrom::Start(resume_pos)).await?;
            file
        } else {
            File::create(local_path).await?
        };

        Ok((resume_pos, file))
    }

    /// Download file with resume support
    async fn download_with_resume<C>(
        &mut self,
        remote_path: &str,
        mut file: File,
        resume_pos: u64,
        total_size: Option<u64>,
        callback: Option<&C>,
    ) -> DownloadResult<u64>
    where
        C: FtpProgressCallback + ?Sized,
    {
        let stream = self
            .stream
            .as_mut()
            .ok_or_else(|| DownloadError::NetworkError("Not connected".to_string()))?;

        let start_time = Instant::now();
        let mut bytes_written = resume_pos;

        // Set resume position if needed
        if resume_pos > 0 {
            debug!("Setting restart position to {}", resume_pos);
            let offset: usize = resume_pos
                .try_into()
                .map_err(|_| DownloadError::ResumeError("Resume offset overflow".to_string()))?;
            let restart_future = stream.resume_transfer(offset);
            timeout(self.config.timeout, restart_future)
                .await
                .map_err(|_| DownloadError::Timeout)?
                .map_err(|e| {
                    DownloadError::ResumeError(format!("Failed to set restart position: {}", e))
                })?;
        }

        // Start transfer
        let retrieve_future = stream.retr_as_stream(remote_path);
        let mut data_stream = timeout(self.config.timeout, retrieve_future)
            .await
            .map_err(|_| DownloadError::Timeout)?
            .map_err(|e| DownloadError::FtpError(format!("Failed to start transfer: {}", e)))?;

        // Read and write data in chunks
        let mut buffer = vec![0u8; CHUNK_SIZE];
        let mut last_progress = Instant::now();

        loop {
            let bytes_read = match timeout(self.config.timeout, data_stream.read(&mut buffer)).await
            {
                Ok(Ok(0)) => break, // EOF
                Ok(Ok(n)) => n,
                Ok(Err(e)) => {
                    return Err(DownloadError::NetworkError(format!("Read error: {}", e)))
                }
                Err(_) => return Err(DownloadError::Timeout),
            };

            file.write_all(&buffer[..bytes_read]).await?;
            bytes_written += bytes_read as u64;

            // Report progress every second or so
            if let Some(callback) = callback {
                let now = Instant::now();
                if now.duration_since(last_progress) >= Duration::from_millis(500) {
                    callback
                        .on_progress(bytes_written, total_size, start_time.elapsed())
                        .await;
                    last_progress = now;
                }
            }

            // Check if we've reached the expected file size
            if let Some(total) = total_size {
                if bytes_written >= total {
                    debug!("Reached expected file size, stopping download");
                    break;
                }
            }
        }

        file.sync_all().await?;

        // Final progress update
        if let Some(callback) = callback {
            callback
                .on_progress(bytes_written, total_size, start_time.elapsed())
                .await;
        }

        Ok(bytes_written)
    }

    /// Disconnect from the server
    pub async fn disconnect(&mut self) -> DownloadResult<()> {
        if let Some(mut stream) = self.stream.take() {
            debug!("Disconnecting from FTP server");

            let quit_future = stream.quit();
            if let Err(e) = timeout(Duration::from_secs(5), quit_future).await {
                warn!("Timeout during disconnect: {}", e);
            }
        }

        self.connected = false;
        Ok(())
    }

    /// Check if connected
    pub fn is_connected(&self) -> bool {
        self.connected
    }

    /// Get current directory
    pub fn current_directory(&self) -> &str {
        &self.current_dir
    }
}

impl Drop for FtpClient {
    fn drop(&mut self) {
        if self.connected {
            warn!("FTP client dropped while still connected");
        }
    }
}

/// Utility function to create a temporary download filename
pub fn get_download_filename(local_name: &str, file_size: u64) -> PathBuf {
    let mut temp_name = local_name.replace(['\\', '.', ' '], "_");
    temp_name = format!("{}_{}.tmp", temp_name, file_size);

    PathBuf::from("download").join(temp_name)
}

/// Prepare directories for a file path
pub async fn prepare_directories(file_path: &Path) -> DownloadResult<()> {
    if let Some(parent) = file_path.parent() {
        tokio::fs::create_dir_all(parent).await?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    struct TestCallback;

    #[async_trait]
    impl FtpProgressCallback for TestCallback {
        async fn on_progress(&self, bytes_read: u64, total_size: Option<u64>, elapsed: Duration) {
            println!(
                "Progress: {}/{:?} bytes in {:?}",
                bytes_read, total_size, elapsed
            );
        }

        async fn on_status_change(&self, status: DownloadStatus) {
            println!("Status changed to: {:?}", status);
        }

        async fn on_error(&self, error: &DownloadError) {
            println!("Error: {}", error);
        }

        async fn query_resume(&self) -> bool {
            true
        }
    }

    #[test]
    fn test_download_filename() {
        let filename = get_download_filename("path\\to\\file.txt", 1024);
        assert_eq!(
            filename,
            PathBuf::from("download/path_to_file_txt_1024.tmp")
        );
    }

    #[tokio::test]
    async fn test_prepare_directories() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("subdir").join("file.txt");

        prepare_directories(&file_path).await.unwrap();

        assert!(file_path.parent().unwrap().exists());
    }

    // TODO: Add integration tests with a test FTP server
}
