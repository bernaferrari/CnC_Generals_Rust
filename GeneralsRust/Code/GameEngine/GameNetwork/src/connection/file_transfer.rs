//! File transfer coordination for multiplayer connections
//!
//! This module handles file transfer coordination between players in a multiplayer
//! game, supporting map files, saves, and other game assets that need to be
//! synchronized across all players.

use crate::commands::{NetCommand, NetCommandType, CommandPayload};
use crate::error::{NetworkError, NetworkResult};
use crate::time::NetworkInstant;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, VecDeque};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;
use tokio::fs::{File, OpenOptions};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::sync::{broadcast, mpsc, RwLock, Semaphore};
use tokio::task::JoinHandle;
use log;
use uuid::Uuid;
use ring::digest::{Context, SHA256};

/// File transfer configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileTransferConfig {
    /// Maximum concurrent transfers
    pub max_concurrent_transfers: usize,
    /// Chunk size for transfer (bytes)
    pub chunk_size: usize,
    /// Transfer timeout
    pub transfer_timeout: Duration,
    /// Maximum file size that can be transferred
    pub max_file_size: u64,
    /// Allowed file extensions
    pub allowed_extensions: Vec<String>,
    /// Transfer directory for temporary files
    pub transfer_directory: PathBuf,
    /// Enable compression for transfers
    pub enable_compression: bool,
    /// Enable checksum verification
    pub enable_checksums: bool,
}

impl Default for FileTransferConfig {
    fn default() -> Self {
        Self {
            max_concurrent_transfers: 4,
            chunk_size: 476 - 6, // Match C++ packet size minus header (476 - CRC(4) - Magic(2))
            transfer_timeout: Duration::from_secs(120), // 120 seconds = 120000ms (C++ FileTransfer.cpp)
            max_file_size: 100 * 1024 * 1024, // 100MB
            allowed_extensions: vec![
                ".map".to_string(),
                ".sav".to_string(),
                ".rep".to_string(),
                ".big".to_string(),
                ".ini".to_string(),
                ".txt".to_string(),
            ],
            transfer_directory: PathBuf::from("./transfers"),
            enable_compression: true,
            enable_checksums: true,
        }
    }
}

/// File transfer state
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FileTransferState {
    /// Transfer is being initiated
    Initiating,
    /// Transfer announcement sent/received
    Announced,
    /// Transfer in progress
    InProgress,
    /// Transfer completed successfully
    Completed,
    /// Transfer failed
    Failed,
    /// Transfer cancelled
    Cancelled,
    /// Transfer paused
    Paused,
}

/// File transfer priority
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum TransferPriority {
    /// Low priority - background transfers
    Low = 0,
    /// Normal priority - regular files
    Normal = 1,
    /// High priority - critical game files
    High = 2,
    /// Critical priority - required for game start
    Critical = 3,
}

/// File chunk information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileChunk {
    /// Transfer ID
    pub transfer_id: Uuid,
    /// Chunk sequence number
    pub chunk_number: u32,
    /// Total number of chunks
    pub total_chunks: u32,
    /// Chunk data
    pub data: Vec<u8>,
    /// Chunk checksum (if enabled)
    pub checksum: Option<u32>,
}

/// File transfer metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileTransferMetadata {
    /// Transfer unique identifier
    pub transfer_id: Uuid,
    /// Original filename
    pub filename: String,
    /// File size in bytes
    pub file_size: u64,
    /// File checksum (full file)
    pub file_checksum: Option<[u8; 32]>,
    /// Transfer priority
    pub priority: TransferPriority,
    /// Sender player ID
    pub sender_player: u8,
    /// Recipient player IDs (empty = broadcast to all)
    pub recipients: Vec<u8>,
    /// Transfer description/purpose
    pub description: Option<String>,
    /// File modification time
    pub modified_time: Option<std::time::SystemTime>,
}

/// Active file transfer tracking
#[derive(Debug)]
pub struct ActiveTransfer {
    /// Transfer metadata
    pub metadata: FileTransferMetadata,
    /// Current state
    pub state: FileTransferState,
    /// Transfer start time
    pub started_at: NetworkInstant,
    /// Last activity time
    pub last_activity: NetworkInstant,
    /// Bytes transferred
    pub bytes_transferred: u64,
    /// Transfer rate (bytes per second)
    pub transfer_rate: f64,
    /// File handle for reading/writing
    pub file_handle: Option<File>,
    /// Chunks received/sent
    pub chunks_processed: u32,
    /// Missing/pending chunks
    pub pending_chunks: VecDeque<u32>,
    /// Participants status
    pub participant_status: HashMap<u8, FileTransferState>,
    /// Error message if failed
    pub error_message: Option<String>,
}

impl ActiveTransfer {
    /// Create new active transfer
    pub fn new(metadata: FileTransferMetadata) -> Self {
        let chunk_size = 476 - 6; // C++ packet size minus header
        let total_chunks = ((metadata.file_size as f64) / chunk_size as f64).ceil() as u32;
        let mut pending_chunks = VecDeque::new();
        
        // Initialize pending chunks queue
        for chunk_num in 0..total_chunks {
            pending_chunks.push_back(chunk_num);
        }

        let mut participant_status = HashMap::new();
        for &player_id in &metadata.recipients {
            participant_status.insert(player_id, FileTransferState::Announced);
        }

        let now = NetworkInstant::now();
        Self {
            metadata,
            state: FileTransferState::Initiating,
            started_at: now,
            last_activity: now,
            bytes_transferred: 0,
            transfer_rate: 0.0,
            file_handle: None,
            chunks_processed: 0,
            pending_chunks,
            participant_status,
            error_message: None,
        }
    }

    /// Update transfer progress
    pub fn update_progress(&mut self, bytes_transferred: u64) {
        let now = NetworkInstant::now();
        let elapsed = now.duration_since(self.started_at).as_secs_f64();
        
        self.bytes_transferred = bytes_transferred;
        self.last_activity = now;
        
        if elapsed > 0.0 {
            self.transfer_rate = bytes_transferred as f64 / elapsed;
        }
    }

    /// Get transfer progress percentage
    pub fn get_progress_percent(&self) -> f64 {
        if self.metadata.file_size == 0 {
            return 100.0;
        }
        (self.bytes_transferred as f64 / self.metadata.file_size as f64) * 100.0
    }

    /// Check if transfer is stale (no activity for too long)
    pub fn is_stale(&self, timeout: Duration) -> bool {
        self.last_activity.elapsed() > timeout
    }

    /// Mark transfer as failed
    pub fn mark_failed(&mut self, error: String) {
        self.state = FileTransferState::Failed;
        self.error_message = Some(error);
        self.last_activity = NetworkInstant::now();
    }

    /// Get estimated time remaining
    pub fn estimated_time_remaining(&self) -> Duration {
        if self.transfer_rate <= 0.0 {
            return Duration::from_secs(u64::MAX);
        }
        
        let remaining_bytes = self.metadata.file_size.saturating_sub(self.bytes_transferred);
        let estimated_seconds = remaining_bytes as f64 / self.transfer_rate;
        Duration::from_secs_f64(estimated_seconds)
    }
}

/// File transfer coordinator
pub struct FileTransferCoordinator {
    /// Configuration
    config: FileTransferConfig,
    
    /// Active transfers
    active_transfers: Arc<RwLock<HashMap<Uuid, ActiveTransfer>>>,
    
    /// Transfer priority queue
    priority_queue: Arc<RwLock<VecDeque<Uuid>>>,
    
    /// Transfer statistics
    stats: Arc<RwLock<TransferStats>>,
    
    /// Semaphore for limiting concurrent transfers
    transfer_semaphore: Arc<Semaphore>,
    
    /// Background task handles
    background_tasks: Vec<JoinHandle<()>>,
    
    /// Communication channels
    transfer_events_tx: broadcast::Sender<TransferEvent>,
    chunk_queue_tx: mpsc::Sender<(u8, FileChunk)>,
    chunk_queue_rx: Arc<RwLock<mpsc::Receiver<(u8, FileChunk)>>>,
    
    /// Shutdown coordination
    shutdown_tx: broadcast::Sender<()>,
}

/// Transfer event notifications
#[derive(Debug, Clone)]
pub enum TransferEvent {
    /// Transfer started
    Started {
        transfer_id: Uuid,
        metadata: FileTransferMetadata,
    },
    /// Transfer progress update
    Progress {
        transfer_id: Uuid,
        bytes_transferred: u64,
        total_bytes: u64,
    },
    /// Transfer completed
    Completed {
        transfer_id: Uuid,
        duration: Duration,
    },
    /// Transfer failed
    Failed {
        transfer_id: Uuid,
        error: String,
    },
    /// Transfer cancelled
    Cancelled {
        transfer_id: Uuid,
        reason: String,
    },
}

/// Transfer statistics
#[derive(Debug, Clone, Default)]
pub struct TransferStats {
    /// Total transfers initiated
    pub transfers_initiated: u64,
    /// Total transfers completed
    pub transfers_completed: u64,
    /// Total transfers failed
    pub transfers_failed: u64,
    /// Total bytes transferred
    pub total_bytes_transferred: u64,
    /// Average transfer rate (bytes per second)
    pub average_transfer_rate: f64,
    /// Currently active transfers
    pub active_transfers: usize,
    /// Total chunks processed
    pub chunks_processed: u64,
    /// Duplicate chunks received
    pub duplicate_chunks: u64,
}

impl FileTransferCoordinator {
    /// Create new file transfer coordinator
    pub fn new() -> Self {
        Self::with_config(FileTransferConfig::default())
    }

    /// Create with custom configuration
    pub fn with_config(config: FileTransferConfig) -> Self {
        let (transfer_events_tx, _) = broadcast::channel(1000);
        let (chunk_queue_tx, chunk_queue_rx) = mpsc::channel(10000);
        let (shutdown_tx, _) = broadcast::channel(1);

        Self {
            transfer_semaphore: Arc::new(Semaphore::new(config.max_concurrent_transfers)),
            config,
            active_transfers: Arc::new(RwLock::new(HashMap::new())),
            priority_queue: Arc::new(RwLock::new(VecDeque::new())),
            stats: Arc::new(RwLock::new(TransferStats::default())),
            background_tasks: Vec::new(),
            transfer_events_tx,
            chunk_queue_tx,
            chunk_queue_rx: Arc::new(RwLock::new(chunk_queue_rx)),
            shutdown_tx,
        }
    }

    /// Start the file transfer coordinator
    pub async fn start(&mut self) -> NetworkResult<()> {
        info!("Starting file transfer coordinator");

        // Create transfer directory if it doesn't exist
        if let Err(e) = tokio::fs::create_dir_all(&self.config.transfer_directory).await {
            warn!("Failed to create transfer directory: {}", e);
        }

        // Start background processing task
        self.start_processing_task().await?;

        // Start cleanup task
        self.start_cleanup_task().await?;

        info!("File transfer coordinator started");
        Ok(())
    }

    /// Initiate a file transfer
    pub async fn initiate_transfer<P: AsRef<Path>>(
        &self,
        file_path: P,
        sender_player: u8,
        recipients: Vec<u8>,
        priority: TransferPriority,
        description: Option<String>,
    ) -> NetworkResult<Uuid> {
        let file_path = file_path.as_ref();
        
        // Validate file
        self.validate_file_for_transfer(file_path).await?;
        
        // Get file metadata
        let file_metadata = tokio::fs::metadata(file_path).await
            .map_err(|e| NetworkError::generic(format!("failed to read file metadata: {}", e)))?;
        
        let file_size = file_metadata.len();
        if file_size > self.config.max_file_size {
            return Err(NetworkError::generic(format!(
                "file too large: {} bytes (max: {} bytes)",
                file_size, self.config.max_file_size
            )));
        }

        // Calculate checksum if enabled
        let file_checksum = if self.config.enable_checksums {
            Some(self.calculate_file_checksum(file_path).await?)
        } else {
            None
        };

        // Create transfer metadata
        let transfer_id = Uuid::new_v4();
        let metadata = FileTransferMetadata {
            transfer_id,
            filename: file_path.file_name()
                .and_then(|name| name.to_str())
                .unwrap_or("unknown")
                .to_string(),
            file_size,
            file_checksum,
            priority,
            sender_player,
            recipients: recipients.clone(),
            description,
            modified_time: file_metadata.modified().ok(),
        };

        // Create active transfer
        let mut active_transfer = ActiveTransfer::new(metadata.clone());
        
        // Open file for reading
        let file = File::open(file_path).await
            .map_err(|e| NetworkError::generic(format!("failed to open file: {}", e)))?;
        active_transfer.file_handle = Some(file);

        // Add to active transfers
        {
            let mut transfers = self.active_transfers.write().await;
            transfers.insert(transfer_id, active_transfer);
        }

        // Add to priority queue
        {
            let mut queue = self.priority_queue.write().await;
            // Insert based on priority (higher priority goes first)
            let mut inserted = false;
            for (i, &existing_id) in queue.iter().enumerate() {
                let existing_transfer = {
                    let transfers = self.active_transfers.read().await;
                    transfers.get(&existing_id).map(|t| t.metadata.priority).unwrap_or(TransferPriority::Low)
                };
                
                if priority > existing_transfer {
                    queue.insert(i, transfer_id);
                    inserted = true;
                    break;
                }
            }
            
            if !inserted {
                queue.push_back(transfer_id);
            }
        }

        // Update statistics
        {
            let mut stats = self.stats.write().await;
            stats.transfers_initiated += 1;
            stats.active_transfers += 1;
        }

        // Send start event
        let _ = self.transfer_events_tx.send(TransferEvent::Started {
            transfer_id,
            metadata,
        });

        info!("Initiated file transfer {} for file: {:?}", transfer_id, file_path);
        Ok(transfer_id)
    }

    /// Process incoming file chunk
    pub async fn process_chunk(&self, sender_player: u8, chunk: FileChunk) -> NetworkResult<()> {
        let transfer_id = chunk.transfer_id;
        
        // Find active transfer
        let mut transfer_found = false;
        {
            let mut transfers = self.active_transfers.write().await;
            if let Some(active_transfer) = transfers.get_mut(&transfer_id) {
                transfer_found = true;
                
                // Verify chunk
                if self.config.enable_checksums {
                    if let Some(expected_checksum) = chunk.checksum {
                        let actual_checksum = self.calculate_chunk_checksum(&chunk.data);
                        if actual_checksum != expected_checksum {
                            active_transfer.mark_failed("chunk checksum mismatch".to_string());
                            return Err(NetworkError::generic("chunk checksum verification failed"));
                        }
                    }
                }
                
                // Write chunk to file
                if let Some(ref mut file) = active_transfer.file_handle {
                    // Seek to correct position
                    let chunk_offset = chunk.chunk_number as u64 * self.config.chunk_size as u64;
                    
                    // Write chunk data
                    if let Err(e) = file.write_all(&chunk.data).await {
                        active_transfer.mark_failed(format!("write error: {}", e));
                        return Err(NetworkError::generic(format!("failed to write chunk: {}", e)));
                    }
                }
                
                // Update progress
                active_transfer.chunks_processed += 1;
                active_transfer.update_progress(
                    active_transfer.chunks_processed as u64 * self.config.chunk_size as u64
                );
                
                // Remove from pending chunks
                if let Some(pos) = active_transfer.pending_chunks
                    .iter()
                    .position(|&chunk_num| chunk_num == chunk.chunk_number)
                {
                    active_transfer.pending_chunks.remove(pos);
                }
                
                // Check if transfer is complete
                if active_transfer.pending_chunks.is_empty() {
                    active_transfer.state = FileTransferState::Completed;
                    
                    // Send completion event
                    let duration = active_transfer.started_at.elapsed();
                    let _ = self.transfer_events_tx.send(TransferEvent::Completed {
                        transfer_id,
                        duration,
                    });
                    
                    info!("File transfer {} completed in {:?}", transfer_id, duration);
                } else {
                    // Send progress event
                    let _ = self.transfer_events_tx.send(TransferEvent::Progress {
                        transfer_id,
                        bytes_transferred: active_transfer.bytes_transferred,
                        total_bytes: active_transfer.metadata.file_size,
                    });
                }
            }
        }
        
        if !transfer_found {
            warn!("Received chunk for unknown transfer: {}", transfer_id);
            return Err(NetworkError::generic("unknown transfer"));
        }
        
        // Update statistics
        {
            let mut stats = self.stats.write().await;
            stats.chunks_processed += 1;
        }
        
        trace!("Processed chunk {} for transfer {}", chunk.chunk_number, transfer_id);
        Ok(())
    }

    /// Cancel a file transfer
    pub async fn cancel_transfer(&self, transfer_id: Uuid, reason: String) -> NetworkResult<()> {
        let mut found = false;
        
        {
            let mut transfers = self.active_transfers.write().await;
            if let Some(active_transfer) = transfers.get_mut(&transfer_id) {
                active_transfer.state = FileTransferState::Cancelled;
                active_transfer.error_message = Some(reason.clone());
                found = true;
            }
        }
        
        if found {
            // Remove from priority queue
            {
                let mut queue = self.priority_queue.write().await;
                queue.retain(|&id| id != transfer_id);
            }
            
            // Send cancellation event
            let _ = self.transfer_events_tx.send(TransferEvent::Cancelled {
                transfer_id,
                reason,
            });
            
            info!("Cancelled file transfer: {}", transfer_id);
        }
        
        Ok(())
    }

    /// Get transfer statistics
    pub async fn get_stats(&self) -> TransferStats {
        let stats = self.stats.read().await.clone();
        stats
    }

    /// Get active transfer information
    pub async fn get_active_transfers(&self) -> Vec<(Uuid, FileTransferState, f64)> {
        let transfers = self.active_transfers.read().await;
        transfers.iter()
            .map(|(&id, transfer)| (id, transfer.state, transfer.get_progress_percent()))
            .collect()
    }

    /// Subscribe to transfer events
    pub fn subscribe_events(&self) -> broadcast::Receiver<TransferEvent> {
        self.transfer_events_tx.subscribe()
    }

    /// Validate file for transfer
    async fn validate_file_for_transfer<P: AsRef<Path>>(&self, file_path: P) -> NetworkResult<()> {
        let file_path = file_path.as_ref();
        
        // Check if file exists
        if !file_path.exists() {
            return Err(NetworkError::generic("file does not exist"));
        }
        
        // Check file extension
        if let Some(extension) = file_path.extension().and_then(|ext| ext.to_str()) {
            let extension = format!(".{}", extension.to_ascii_lowercase());
            if !self.config.allowed_extensions.contains(&extension) {
                return Err(NetworkError::generic(format!("file extension not allowed: {}", extension)));
            }
        } else {
            return Err(NetworkError::generic("file has no extension"));
        }
        
        Ok(())
    }

    /// Calculate file checksum
    async fn calculate_file_checksum<P: AsRef<Path>>(
        &self,
        file_path: P,
    ) -> NetworkResult<[u8; 32]> {
        let mut file = File::open(file_path)
            .await
            .map_err(|e| NetworkError::generic(format!("failed to open file for checksum: {}", e)))?;

        let mut ctx = Context::new(&SHA256);
        let mut buffer = vec![0u8; 8192];

        loop {
            let bytes_read = file
                .read(&mut buffer)
                .await
                .map_err(|e| NetworkError::generic(format!("failed to read file for checksum: {}", e)))?;

            if bytes_read == 0 {
                break;
            }

            ctx.update(&buffer[..bytes_read]);
        }

        let digest = ctx.finish();
        let mut out = [0u8; 32];
        out.copy_from_slice(digest.as_ref());
        Ok(out)
    }

    /// Calculate chunk checksum
    fn calculate_chunk_checksum(&self, data: &[u8]) -> u32 {
        crc32fast::hash(data)
    }

    /// Start chunk processing task
    async fn start_processing_task(&mut self) -> NetworkResult<()> {
        let chunk_queue_rx = self.chunk_queue_rx.clone();
        let active_transfers = self.active_transfers.clone();
        let config = self.config.clone();
        let stats = self.stats.clone();
        let mut shutdown_rx = self.shutdown_tx.subscribe();

        let handle = tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_millis(10));

            loop {
                tokio::select! {
                    _ = interval.tick() => {
                        // Process pending chunks
                        // This would coordinate chunk sending/receiving
                        // Implementation depends on integration with connection system
                    }
                    _ = shutdown_rx.recv() => {
                        debug!("File transfer processing task shutting down");
                        break;
                    }
                }
            }
        });

        self.background_tasks.push(handle);
        Ok(())
    }

    /// Start cleanup task for stale transfers
    async fn start_cleanup_task(&mut self) -> NetworkResult<()> {
        let active_transfers = self.active_transfers.clone();
        let config = self.config.clone();
        let transfer_events_tx = self.transfer_events_tx.clone();
        let mut shutdown_rx = self.shutdown_tx.subscribe();

        let handle = tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(30));

            loop {
                tokio::select! {
                    _ = interval.tick() => {
                        let mut to_remove = Vec::new();
                        
                        {
                            let mut transfers = active_transfers.write().await;
                            
                            for (&transfer_id, transfer) in transfers.iter_mut() {
                                if transfer.is_stale(config.transfer_timeout) {
                                    transfer.mark_failed("transfer timeout".to_string());
                                    to_remove.push(transfer_id);
                                    
                                    // Send failure event
                                    let _ = transfer_events_tx.send(TransferEvent::Failed {
                                        transfer_id,
                                        error: "transfer timeout".to_string(),
                                    });
                                }
                            }
                            
                            // Remove stale transfers
                            for transfer_id in &to_remove {
                                transfers.remove(transfer_id);
                            }
                        }
                        
                        if !to_remove.is_empty() {
                            debug!("Cleaned up {} stale transfers", to_remove.len());
                        }
                    }
                    _ = shutdown_rx.recv() => {
                        debug!("File transfer cleanup task shutting down");
                        break;
                    }
                }
            }
        });

        self.background_tasks.push(handle);
        Ok(())
    }

    /// Shutdown the coordinator
    pub async fn shutdown(&mut self) -> NetworkResult<()> {
        info!("Shutting down file transfer coordinator");

        // Send shutdown signal
        let _ = self.shutdown_tx.send(());

        // Wait for background tasks
        for handle in self.background_tasks.drain(..) {
            handle.abort();
            let _ = handle.await;
        }

        // Close all active transfers
        {
            let mut transfers = self.active_transfers.write().await;
            for (transfer_id, transfer) in transfers.drain() {
                if transfer.state == FileTransferState::InProgress {
                    let _ = self.transfer_events_tx.send(TransferEvent::Cancelled {
                        transfer_id,
                        reason: "coordinator shutdown".to_string(),
                    });
                }
            }
        }

        info!("File transfer coordinator shutdown complete");
        Ok(())
    }
}

impl Default for FileTransferCoordinator {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::io::AsyncWriteExt;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_coordinator_creation() {
        let coordinator = FileTransferCoordinator::new();
        let stats = coordinator.get_stats().await;
        
        assert_eq!(stats.transfers_initiated, 0);
        assert_eq!(stats.active_transfers, 0);
    }

    #[tokio::test]
    async fn test_file_validation() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("test.map");
        
        // Create test file
        let mut file = tokio::fs::File::create(&file_path).await.unwrap();
        file.write_all(b"test content").await.unwrap();
        
        let coordinator = FileTransferCoordinator::new();
        let result = coordinator.validate_file_for_transfer(&file_path).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_active_transfer_creation() {
        let metadata = FileTransferMetadata {
            transfer_id: Uuid::new_v4(),
            filename: "test.map".to_string(),
            file_size: 1000,
            file_checksum: None,
            priority: TransferPriority::Normal,
            sender_player: 0,
            recipients: vec![1, 2],
            description: None,
            modified_time: None,
        };
        
        let transfer = ActiveTransfer::new(metadata);
        assert_eq!(transfer.state, FileTransferState::Initiating);
        assert_eq!(transfer.get_progress_percent(), 0.0);
    }

    #[tokio::test]
    async fn test_chunk_processing() {
        let mut coordinator = FileTransferCoordinator::new();
        coordinator.start().await.unwrap();
        
        let chunk = FileChunk {
            transfer_id: Uuid::new_v4(),
            chunk_number: 0,
            total_chunks: 1,
            data: vec![1, 2, 3, 4],
            checksum: None,
        };
        
        // This would fail because transfer doesn't exist, but tests the error path
        let result = coordinator.process_chunk(0, chunk).await;
        assert!(result.is_err());
    }
}
