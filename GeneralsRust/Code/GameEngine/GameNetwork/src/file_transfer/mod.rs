//! File transfer module
//!
//! Provides async file transfer system for maps, mods, and other game assets.
//! Includes bandwidth throttling, resume capability, and checksum validation.

pub mod bandwidth;
pub mod metadata;

use crate::error::{NetworkError, NetworkResult};
use crate::observability::TransferTelemetryDirection;
use crate::time::NetworkInstant;
use crate::transport::Transport;
use ring::digest::{Context, SHA256};
use std::collections::HashMap;
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use tokio::fs;
use tokio::io::AsyncReadExt;
use uuid::Uuid;

// Re-export bandwidth types
pub use bandwidth::{BandwidthManager, BandwidthStats, BandwidthThrottle, ThrottleStats};

// Re-export metadata types
pub use metadata::{FileMetadata, TransferType};

/// Direction of the file transfer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransferDirection {
    Upload,
    Download,
}

impl From<TransferDirection> for TransferTelemetryDirection {
    fn from(direction: TransferDirection) -> Self {
        match direction {
            TransferDirection::Upload => TransferTelemetryDirection::Upload,
            TransferDirection::Download => TransferTelemetryDirection::Download,
        }
    }
}

/// File transfer configuration.
#[derive(Debug, Clone)]
pub struct TransferConfig {
    /// Incoming directory for received files.
    pub incoming_directory: PathBuf,
    /// Outgoing directory for sent files.
    pub outgoing_directory: PathBuf,
    /// Chunk size for streaming operations.
    pub chunk_size: usize,
    /// Maximum file size allowed.
    pub max_file_size: u64,
    /// Max concurrent transfers tracked.
    pub max_concurrent_transfers: usize,
}

impl Default for TransferConfig {
    fn default() -> Self {
        let base = std::env::temp_dir().join("generals_file_transfer");
        Self {
            incoming_directory: base.join("incoming"),
            outgoing_directory: base.join("outgoing"),
            chunk_size: 128 * 1024,           // 128KB default
            max_file_size: 512 * 1024 * 1024, // 512MB safety cap
            max_concurrent_transfers: 8,
        }
    }
}

/// Transfer progress snapshot.
#[derive(Debug, Clone)]
pub struct TransferProgress {
    pub transfer_id: Uuid,
    pub metadata: FileMetadata,
    pub direction: TransferDirection,
    pub bytes_transferred: u64,
    pub complete: bool,
    pub peer: Option<SocketAddr>,
    pub started_at: NetworkInstant,
}

impl TransferProgress {
    pub fn new(
        transfer_id: Uuid,
        metadata: FileMetadata,
        direction: TransferDirection,
        peer: Option<SocketAddr>,
    ) -> Self {
        Self {
            transfer_id,
            metadata,
            direction,
            bytes_transferred: 0,
            complete: false,
            peer,
            started_at: NetworkInstant::now(),
        }
    }

    pub fn update(&mut self, bytes_transferred: u64) {
        self.bytes_transferred = bytes_transferred.min(self.metadata.file_size);
        if self.bytes_transferred >= self.metadata.file_size {
            self.complete = true;
        }
    }

    pub fn set_complete(&mut self) {
        self.complete = true;
        self.bytes_transferred = self.metadata.file_size;
    }
}

/// Callback interface for transfer lifecycle events.
pub trait ProgressCallback: Send + Sync {
    fn on_started(&self, progress: &TransferProgress);
    fn on_progress(&self, progress: &TransferProgress);
    fn on_completed(&self, progress: &TransferProgress);
    fn on_failed(&self, progress: &TransferProgress, error: &NetworkError);
}

/// Mutable bookkeeping state of the transfer manager.
struct TransferState {
    config: TransferConfig,
    uploads: HashMap<Uuid, TransferProgress>,
    downloads: HashMap<Uuid, TransferProgress>,
    callback: Option<Arc<dyn ProgressCallback>>,
}

/// Minimal file transfer manager (inspection + bookkeeping).
pub struct FileTransferManager {
    transport: Arc<Transport>,
    /// THREAD: single guarded bundle for config, transfer progress and the progress
    /// callback. The manager is published as `Arc<FileTransferManager>` and inspect /
    /// query calls arrive from arbitrary tasks while configuration runs on the
    /// owning thread. Critical sections are synchronous; no guard is held across an
    /// `.await`.
    state: Mutex<TransferState>,
}

impl FileTransferManager {
    pub fn new(transport: Arc<Transport>) -> Arc<Self> {
        Self::with_config(transport, TransferConfig::default())
    }

    pub fn with_config(transport: Arc<Transport>, config: TransferConfig) -> Arc<Self> {
        Arc::new(Self {
            transport,
            state: Mutex::new(TransferState {
                config,
                uploads: HashMap::new(),
                downloads: HashMap::new(),
                callback: None,
            }),
        })
    }

    fn lock_state(&self) -> std::sync::MutexGuard<'_, TransferState> {
        self.state
            .lock()
            .expect("FileTransferManager state lock poisoned")
    }

    pub fn set_progress_callback(&self, callback: Arc<dyn ProgressCallback>) {
        self.lock_state().callback = Some(callback);
    }

    pub fn set_incoming_directory<P: AsRef<Path>>(&self, path: P) -> NetworkResult<()> {
        let path_ref = path.as_ref();
        std::fs::create_dir_all(path_ref).map_err(|e| {
            NetworkError::generic(format!("failed to create incoming directory: {}", e))
        })?;
        self.lock_state().config.incoming_directory = path_ref.to_path_buf();
        Ok(())
    }

    pub async fn uploads(&self) -> Vec<TransferProgress> {
        let state = self.lock_state();
        state.uploads.values().cloned().collect()
    }

    pub async fn downloads(&self) -> Vec<TransferProgress> {
        let state = self.lock_state();
        state.downloads.values().cloned().collect()
    }

    pub async fn shutdown(&self) {
        let mut state = self.lock_state();
        state.uploads.clear();
        state.downloads.clear();
    }

    pub async fn inspect_file<P: AsRef<Path>>(
        &self,
        path: P,
        transfer_type: TransferType,
    ) -> NetworkResult<FileMetadata> {
        let path_ref = path.as_ref();
        let filename = path_ref
            .file_name()
            .and_then(|name| name.to_str())
            .ok_or_else(|| NetworkError::generic("invalid filename"))?
            .to_string();

        let metadata = fs::metadata(path_ref)
            .await
            .map_err(|e| NetworkError::generic(format!("failed to stat file: {}", e)))?;

        let (max_file_size, chunk_size) = {
            let state = self.lock_state();
            (state.config.max_file_size, state.config.chunk_size)
        };

        if metadata.len() > max_file_size {
            return Err(NetworkError::generic("file exceeds max transfer size"));
        }

        let checksum = self.compute_checksum(path_ref, chunk_size).await?;

        Ok(FileMetadata {
            filename,
            file_size: metadata.len(),
            checksum,
            transfer_type,
        })
    }

    async fn compute_checksum<P: AsRef<Path>>(
        &self,
        path: P,
        chunk_size: usize,
    ) -> NetworkResult<[u8; 32]> {
        let mut file = fs::File::open(path)
            .await
            .map_err(|e| NetworkError::generic(format!("failed to open file: {}", e)))?;
        let mut ctx = Context::new(&SHA256);
        let mut buffer = vec![0u8; chunk_size.max(1)];
        loop {
            let read = file
                .read(&mut buffer)
                .await
                .map_err(|e| NetworkError::generic(format!("failed to read file: {}", e)))?;
            if read == 0 {
                break;
            }
            ctx.update(&buffer[..read]);
        }
        let digest = ctx.finish();
        let mut out = [0u8; 32];
        out.copy_from_slice(digest.as_ref());
        Ok(out)
    }
}
