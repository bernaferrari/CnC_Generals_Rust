//! Simplified download manager implementation

use crate::config::ConfigManager;
use crate::error::{DownloadError, DownloadResult, DownloadStatus};
use crate::ftp_client::{FtpClient, FtpConfig, FtpProgressCallback};
use async_trait::async_trait;
use futures::stream::StreamExt;
use reqwest::Client;
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tokio::fs::{File, OpenOptions};
use tokio::io::{AsyncSeekExt, AsyncWriteExt, SeekFrom};
use tracing::{debug, error, info, warn};
use url::Url;

/// Progress information for downloads
#[derive(Debug, Clone)]
pub struct ProgressInfo {
    pub bytes_downloaded: u64,
    pub total_bytes: Option<u64>,
    pub elapsed_time: Duration,
    pub transfer_rate: f64, // bytes per second
}

impl ProgressInfo {
    /// Calculate completion percentage (0.0 to 1.0)
    pub fn completion_ratio(&self) -> Option<f32> {
        self.total_bytes
            .map(|total| (self.bytes_downloaded as f32) / (total as f32).max(1.0))
    }

    /// Get human-readable transfer rate
    pub fn format_rate(&self) -> String {
        if self.transfer_rate > 1_000_000.0 {
            format!("{:.2} MB/s", self.transfer_rate / 1_000_000.0)
        } else if self.transfer_rate > 1_000.0 {
            format!("{:.2} KB/s", self.transfer_rate / 1_000.0)
        } else {
            format!("{:.0} B/s", self.transfer_rate)
        }
    }
}

/// Trait for download event callbacks
#[async_trait]
pub trait DownloadListener: Send + Sync {
    /// Called when an error occurs
    async fn on_error(&self, error: DownloadError);

    /// Called when download completes successfully
    async fn on_completed(&self);

    /// Called to query if a download should be resumed
    async fn should_resume(&self) -> bool;

    /// Called with progress updates
    async fn on_progress(&self, progress: ProgressInfo);

    /// Called when status changes
    async fn on_status_change(&self, status: DownloadStatus);
}

/// Main download manager
pub struct DownloadManager {
    config_manager: ConfigManager,
    http_client: Client,
    current_status: Arc<Mutex<DownloadStatus>>,
}

impl DownloadManager {
    /// Create a new download manager
    pub fn new() -> DownloadResult<Self> {
        let config_manager = ConfigManager::new()?;

        let http_client = Client::builder()
            .timeout(Duration::from_secs(30))
            .user_agent("CnC-Generals-ZeroHour/1.0")
            .build()
            .map_err(|e| {
                DownloadError::NetworkError(format!("Failed to create HTTP client: {}", e))
            })?;

        Ok(Self {
            config_manager,
            http_client,
            current_status: Arc::new(Mutex::new(DownloadStatus::None)),
        })
    }

    /// Download a file via FTP
    pub async fn download_ftp<P, L>(
        &mut self,
        server: &str,
        username: &str,
        password: &str,
        remote_file: &str,
        local_file: P,
        listener: Option<Arc<L>>,
    ) -> DownloadResult<u64>
    where
        P: AsRef<Path>,
        L: DownloadListener,
    {
        self.set_status(DownloadStatus::Connecting).await;

        let config = FtpConfig {
            server: server.to_string(),
            port: 21,
            username: username.to_string(),
            password: password.to_string(),
            ..Default::default()
        };

        let mut ftp_client = FtpClient::new(config);

        // Connect and login with simple retry logic
        let mut attempt: u32 = 0;
        let connect_result = loop {
            match ftp_client.connect().await {
                Ok(()) => {
                    self.set_status(DownloadStatus::Authenticating).await;
                    match ftp_client.login().await {
                        Ok(()) => break Ok(()),
                        Err(e) => {
                            warn!("FTP login failed: {}", e);
                            if let Some(listener) = &listener {
                                listener.on_error(e.clone()).await;
                            }
                        }
                    }
                }
                Err(e) => {
                    warn!("FTP connection attempt failed: {}", e);
                    if let Some(listener) = &listener {
                        listener.on_error(e.clone()).await;
                    }
                }
            }

            attempt += 1;
            if attempt >= 5 {
                break Err(DownloadError::NetworkError(
                    "Exceeded FTP connection retries".to_string(),
                ));
            }
            tokio::time::sleep(Duration::from_secs(1 << attempt)).await;
        };

        if let Err(err) = connect_result {
            self.set_status(DownloadStatus::Error).await;
            return Err(err);
        }
        info!("Successfully connected to FTP server");

        // Parse remote file path
        let (remote_dir, filename) = self.split_remote_path(remote_file);

        if !remote_dir.is_empty() {
            self.set_status(DownloadStatus::FindingFile).await;
            ftp_client.change_directory(&remote_dir).await?;
        }

        // Start download
        self.set_status(DownloadStatus::Downloading).await;

        let result = ftp_client
            .download_file(
                &filename,
                local_file.as_ref(),
                None::<&dyn FtpProgressCallback>,
            )
            .await;

        // Clean up connection
        if let Err(e) = ftp_client.disconnect().await {
            warn!("Error disconnecting from FTP server: {}", e);
        }

        match result {
            Ok(bytes) => {
                self.set_status(DownloadStatus::Finished).await;
                if let Some(listener) = &listener {
                    listener.on_completed().await;
                }
                info!("FTP download completed: {} bytes", bytes);
                Ok(bytes)
            }
            Err(e) => {
                self.set_status(DownloadStatus::Error).await;
                if let Some(listener) = &listener {
                    listener.on_error(e.clone()).await;
                }
                Err(e)
            }
        }
    }

    /// Download a file via HTTP with resume support
    pub async fn download_http<P, L>(
        &mut self,
        url: &str,
        local_file: P,
        listener: Option<Arc<L>>,
    ) -> DownloadResult<u64>
    where
        P: AsRef<Path>,
        L: DownloadListener,
    {
        let local_path = local_file.as_ref();

        info!(
            "Starting HTTP download: {} -> {}",
            url,
            local_path.display()
        );
        self.set_status(DownloadStatus::Connecting).await;

        // Parse URL
        let _parsed_url = Url::parse(url)?;

        // Check for resume
        let resume_pos = if local_path.exists() {
            let metadata = tokio::fs::metadata(local_path).await?;
            let size = metadata.len();

            if size > 0 {
                if let Some(listener) = &listener {
                    if listener.should_resume().await {
                        info!("Resuming HTTP download from byte {}", size);
                        size
                    } else {
                        0
                    }
                } else {
                    size
                }
            } else {
                0
            }
        } else {
            0
        };

        // Build request with range header for resume
        let mut request = self.http_client.get(url);
        if resume_pos > 0 {
            request = request.header("Range", format!("bytes={}-", resume_pos));
        }

        // Send request
        self.set_status(DownloadStatus::Downloading).await;

        let response = request
            .send()
            .await
            .map_err(|e| DownloadError::HttpError(format!("Request failed: {}", e)))?;

        if !response.status().is_success() && response.status().as_u16() != 206 {
            return Err(DownloadError::HttpError(format!(
                "HTTP error: {}",
                response.status()
            )));
        }

        let total_size = response.content_length().map(|len| len + resume_pos);

        // Open local file
        let mut file = if resume_pos > 0 {
            let mut f = OpenOptions::new()
                .write(true)
                .append(true)
                .open(local_path)
                .await?;
            f.seek(SeekFrom::Start(resume_pos)).await?;
            f
        } else {
            // Create parent directories
            if let Some(parent) = local_path.parent() {
                tokio::fs::create_dir_all(parent).await?;
            }
            File::create(local_path).await?
        };

        // Download with progress tracking
        let mut stream = response.bytes_stream();
        let mut bytes_written = resume_pos;
        let start_time = Instant::now();
        let mut last_progress = Instant::now();

        while let Some(chunk_result) = stream.next().await {
            let chunk = chunk_result
                .map_err(|e| DownloadError::HttpError(format!("Stream error: {}", e)))?;

            file.write_all(&chunk).await?;
            bytes_written += chunk.len() as u64;

            // Update progress
            let now = Instant::now();
            if now.duration_since(last_progress) >= Duration::from_millis(250) {
                let elapsed = start_time.elapsed();
                let rate = if elapsed.as_secs_f64() > 0.0 {
                    (bytes_written - resume_pos) as f64 / elapsed.as_secs_f64()
                } else {
                    0.0
                };

                let progress = ProgressInfo {
                    bytes_downloaded: bytes_written,
                    total_bytes: total_size,
                    elapsed_time: elapsed,
                    transfer_rate: rate,
                };

                if let Some(listener) = &listener {
                    listener.on_progress(progress).await;
                }

                last_progress = now;
            }
        }

        file.sync_all().await?;

        // Final progress update
        let final_progress = ProgressInfo {
            bytes_downloaded: bytes_written,
            total_bytes: total_size,
            elapsed_time: start_time.elapsed(),
            transfer_rate: if start_time.elapsed().as_secs_f64() > 0.0 {
                (bytes_written - resume_pos) as f64 / start_time.elapsed().as_secs_f64()
            } else {
                0.0
            },
        };

        if let Some(listener) = &listener {
            listener.on_progress(final_progress).await;
            listener.on_completed().await;
        }

        self.set_status(DownloadStatus::Finished).await;
        info!("HTTP download completed: {} bytes", bytes_written);
        Ok(bytes_written)
    }

    /// Get current status
    pub fn status(&self) -> DownloadStatus {
        *self.current_status.lock().unwrap()
    }

    /// Set current status
    async fn set_status(&self, status: DownloadStatus) {
        {
            let mut current = self.current_status.lock().unwrap();
            *current = status;
        }
        debug!("Download status changed to: {:?}", status);
    }

    /// Split remote path into directory and filename
    fn split_remote_path(&self, remote_path: &str) -> (String, String) {
        let path = remote_path.replace('\\', "/");
        if let Some(last_slash) = path.rfind('/') {
            let (dir, file) = path.split_at(last_slash);
            (dir.to_string(), file[1..].to_string()) // Skip the '/'
        } else {
            (String::new(), path)
        }
    }
}

/// Simple console logger implementation of DownloadListener
pub struct ConsoleDownloadListener;

#[async_trait]
impl DownloadListener for ConsoleDownloadListener {
    async fn on_error(&self, error: DownloadError) {
        error!("Download error: {}", error);
    }

    async fn on_completed(&self) {
        info!("Download completed successfully!");
    }

    async fn should_resume(&self) -> bool {
        true // Default to resuming
    }

    async fn on_progress(&self, progress: ProgressInfo) {
        if let Some(ratio) = progress.completion_ratio() {
            info!(
                "Progress: {:.1}% ({}) - {}",
                ratio * 100.0,
                progress.format_rate(),
                progress.bytes_downloaded
            );
        } else {
            info!(
                "Downloaded: {} bytes ({})",
                progress.bytes_downloaded,
                progress.format_rate()
            );
        }
    }

    async fn on_status_change(&self, status: DownloadStatus) {
        info!("Status: {:?}", status);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_split_remote_path() {
        let dm = DownloadManager::new().unwrap();

        let (dir, file) = dm.split_remote_path("/path/to/file.txt");
        assert_eq!(dir, "/path/to");
        assert_eq!(file, "file.txt");

        let (dir, file) = dm.split_remote_path("file.txt");
        assert_eq!(dir, "");
        assert_eq!(file, "file.txt");

        let (dir, file) = dm.split_remote_path("path\\to\\file.txt");
        assert_eq!(dir, "path/to");
        assert_eq!(file, "file.txt");
    }
}
