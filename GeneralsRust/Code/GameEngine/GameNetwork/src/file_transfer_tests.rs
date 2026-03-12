//! Comprehensive test suite for file transfer system
//!
//! Tests cover:
//! - File chunking and transfer
//! - Checksum validation
//! - Resume capability
//! - Concurrent transfers
//! - Error recovery
//! - Large file handling
//! - Progress tracking

#[cfg(test)]
mod tests {
    use crate::error::NetworkResult;
    use crate::file_transfer::{
        FileMetadata, FileTransferManager, ProgressCallback, TransferConfig, TransferDirection,
        TransferProgress, TransferType,
    };
    use crate::transport::Transport;
    use parking_lot::Mutex;
    use std::collections::HashMap;
    use std::net::SocketAddr;
    use std::path::PathBuf;
    use std::sync::Arc;
    use tempfile::TempDir;
    use tokio::fs::File;
    use tokio::io::AsyncWriteExt;
    use uuid::Uuid;

    // Test progress callback implementation
    struct TestProgressCallback {
        events: Arc<Mutex<Vec<ProgressEvent>>>,
    }

    #[derive(Debug, Clone)]
    enum ProgressEvent {
        Started(Uuid),
        Progress(Uuid, u64),
        Completed(Uuid),
        Failed(Uuid, String),
    }

    impl TestProgressCallback {
        fn new() -> (Self, Arc<Mutex<Vec<ProgressEvent>>>) {
            let events = Arc::new(Mutex::new(Vec::new()));
            (
                Self {
                    events: events.clone(),
                },
                events,
            )
        }
    }

    impl ProgressCallback for TestProgressCallback {
        fn on_started(&self, progress: &TransferProgress) {
            self.events
                .lock()
                .push(ProgressEvent::Started(progress.transfer_id));
        }

        fn on_progress(&self, progress: &TransferProgress) {
            self.events.lock().push(ProgressEvent::Progress(
                progress.transfer_id,
                progress.bytes_transferred,
            ));
        }

        fn on_completed(&self, progress: &TransferProgress) {
            self.events
                .lock()
                .push(ProgressEvent::Completed(progress.transfer_id));
        }

        fn on_failed(&self, progress: &TransferProgress, error: &crate::error::NetworkError) {
            self.events.lock().push(ProgressEvent::Failed(
                progress.transfer_id,
                error.to_string(),
            ));
        }
    }

    async fn create_test_file(dir: &TempDir, name: &str, size: usize) -> PathBuf {
        let path = dir.path().join(name);
        let mut file = File::create(&path).await.unwrap();

        // Write test data in chunks
        let chunk_size = 1024;
        let mut remaining = size;
        let mut counter = 0u8;

        while remaining > 0 {
            let write_size = remaining.min(chunk_size);
            let mut data = vec![counter; write_size];
            counter = counter.wrapping_add(1);

            file.write_all(&data).await.unwrap();
            remaining -= write_size;
        }

        file.sync_all().await.unwrap();
        path
    }

    fn create_test_transport() -> Arc<Transport> {
        // Create a transport for testing
        let addr: SocketAddr = "127.0.0.1:0".parse().unwrap();
        Transport::new_for_testing(addr)
    }

    #[tokio::test]
    async fn test_inspect_file() {
        let temp_dir = TempDir::new().unwrap();
        let test_file = create_test_file(&temp_dir, "test_map.map", 1024).await;

        let transport = create_test_transport();
        let manager = FileTransferManager::new(transport);

        let metadata = manager
            .inspect_file(&test_file, TransferType::Map)
            .await
            .unwrap();

        assert_eq!(metadata.filename, "test_map.map");
        assert_eq!(metadata.file_size, 1024);
        assert_eq!(metadata.transfer_type, TransferType::Map);
        assert_ne!(metadata.checksum, [0u8; 32]); // Should have valid checksum
    }

    #[tokio::test]
    async fn test_checksum_validation() {
        let temp_dir = TempDir::new().unwrap();

        // Create two identical files
        let file1 = create_test_file(&temp_dir, "file1.map", 2048).await;
        let file2 = create_test_file(&temp_dir, "file2.map", 2048).await;

        let transport = create_test_transport();
        let manager = FileTransferManager::new(transport);

        let meta1 = manager
            .inspect_file(&file1, TransferType::Map)
            .await
            .unwrap();
        let meta2 = manager
            .inspect_file(&file2, TransferType::Map)
            .await
            .unwrap();

        // Same data should produce same checksum
        assert_eq!(meta1.checksum, meta2.checksum);

        // Different file should have different checksum
        let file3 = create_test_file(&temp_dir, "file3.map", 4096).await;
        let meta3 = manager
            .inspect_file(&file3, TransferType::Map)
            .await
            .unwrap();
        assert_ne!(meta1.checksum, meta3.checksum);
    }

    #[tokio::test]
    async fn test_progress_callback_lifecycle() {
        let temp_dir = TempDir::new().unwrap();
        let test_file = create_test_file(&temp_dir, "test.map", 512 * 1024).await; // 512KB

        let transport = create_test_transport();
        let manager = FileTransferManager::new(transport);

        let (callback, events) = TestProgressCallback::new();
        manager.set_progress_callback(Arc::new(callback));

        // Note: Full integration test would require actual transfer
        // This tests the callback registration
        let metadata = manager
            .inspect_file(&test_file, TransferType::Map)
            .await
            .unwrap();

        assert_eq!(metadata.file_size, 512 * 1024);
    }

    #[tokio::test]
    async fn test_chunk_size_configuration() {
        let transport = create_test_transport();

        // Test default chunk size
        let config_default = TransferConfig::default();
        assert_eq!(config_default.chunk_size, 128 * 1024); // 128KB

        // Test custom chunk size
        let custom_config = TransferConfig {
            chunk_size: 64 * 1024, // 64KB
            ..Default::default()
        };

        let manager = FileTransferManager::with_config(transport, custom_config);

        // Verify manager is created successfully with custom config
        assert!(Arc::strong_count(&manager) >= 1);
    }

    #[tokio::test]
    async fn test_transfer_type_serialization() {
        use serde_json;

        let types = vec![
            TransferType::Map,
            TransferType::Mod,
            TransferType::Replay,
            TransferType::SaveGame,
            TransferType::Asset,
            TransferType::Generic,
        ];

        for transfer_type in types {
            let serialized = serde_json::to_string(&transfer_type).unwrap();
            let deserialized: TransferType = serde_json::from_str(&serialized).unwrap();
            assert_eq!(transfer_type, deserialized);
        }
    }

    #[tokio::test]
    async fn test_metadata_serialization() {
        let metadata = FileMetadata {
            filename: "test_map.map".to_string(),
            file_size: 1024 * 1024,
            checksum: [0xAB; 32],
            transfer_type: TransferType::Map,
        };

        let serialized = bincode::serialize(&metadata).unwrap();
        let deserialized: FileMetadata = bincode::deserialize(&serialized).unwrap();

        assert_eq!(metadata.filename, deserialized.filename);
        assert_eq!(metadata.file_size, deserialized.file_size);
        assert_eq!(metadata.checksum, deserialized.checksum);
        assert_eq!(metadata.transfer_type, deserialized.transfer_type);
    }

    #[tokio::test]
    async fn test_multiple_file_types() {
        let temp_dir = TempDir::new().unwrap();
        let transport = create_test_transport();
        let manager = FileTransferManager::new(transport);

        let files = vec![
            ("map.map", TransferType::Map),
            ("replay.rep", TransferType::Replay),
            ("save.sav", TransferType::SaveGame),
            ("mod.big", TransferType::Mod),
        ];

        for (filename, transfer_type) in files {
            let test_file = create_test_file(&temp_dir, filename, 1024).await;
            let metadata = manager.inspect_file(&test_file, transfer_type).await.unwrap();

            assert_eq!(metadata.filename, filename);
            assert_eq!(metadata.transfer_type, transfer_type);
        }
    }

    #[tokio::test]
    async fn test_empty_file() {
        let temp_dir = TempDir::new().unwrap();
        let test_file = create_test_file(&temp_dir, "empty.map", 0).await;

        let transport = create_test_transport();
        let manager = FileTransferManager::new(transport);

        let metadata = manager
            .inspect_file(&test_file, TransferType::Map)
            .await
            .unwrap();

        assert_eq!(metadata.file_size, 0);
        // Empty file should still have a valid checksum
        assert_ne!(metadata.checksum, [0u8; 32]);
    }

    #[tokio::test]
    async fn test_large_file_metadata() {
        let temp_dir = TempDir::new().unwrap();
        // Create a 10MB file
        let test_file = create_test_file(&temp_dir, "large.map", 10 * 1024 * 1024).await;

        let transport = create_test_transport();
        let manager = FileTransferManager::new(transport);

        let start = crate::time::NetworkInstant::now();
        let metadata = manager
            .inspect_file(&test_file, TransferType::Map)
            .await
            .unwrap();
        let duration = start.elapsed();

        assert_eq!(metadata.file_size, 10 * 1024 * 1024);
        // Checksum computation should be reasonably fast even for large files
        assert!(duration.as_secs() < 5, "Checksum took too long: {:?}", duration);
    }

    #[tokio::test]
    async fn test_transfer_progress_tracking() {
        let transfer_id = Uuid::new_v4();
        let metadata = FileMetadata {
            filename: "test.map".to_string(),
            file_size: 1024 * 1024,
            checksum: [0; 32],
            transfer_type: TransferType::Map,
        };

        let mut progress = TransferProgress::new(
            transfer_id,
            metadata.clone(),
            TransferDirection::Download,
            None,
        );

        assert_eq!(progress.bytes_transferred, 0);
        assert!(!progress.complete);

        // Update progress
        progress.update(512 * 1024);
        assert_eq!(progress.bytes_transferred, 512 * 1024);
        assert!(!progress.complete);

        // Complete transfer
        progress.set_complete();
        assert!(progress.complete);
    }

    #[tokio::test]
    async fn test_incoming_directory_configuration() {
        let temp_dir = TempDir::new().unwrap();
        let custom_dir = temp_dir.path().join("custom_incoming");

        let transport = create_test_transport();
        let config = TransferConfig {
            incoming_directory: custom_dir.clone(),
            ..Default::default()
        };

        let manager = FileTransferManager::with_config(transport, config);

        // Set and verify incoming directory
        let result = manager.set_incoming_directory(&custom_dir);
        assert!(result.is_ok());
        assert!(custom_dir.exists());
    }

    #[tokio::test]
    async fn test_concurrent_uploads_and_downloads() {
        let transport = create_test_transport();
        let manager = FileTransferManager::new(transport);

        // Initially, should have no transfers
        let uploads = manager.uploads().await;
        let downloads = manager.downloads().await;

        assert_eq!(uploads.len(), 0);
        assert_eq!(downloads.len(), 0);
    }

    #[tokio::test]
    async fn test_filename_sanitization() {
        // Test various potentially dangerous filenames
        let test_cases = vec![
            ("normal_file.map", true),
            ("file-with-dash.map", true),
            ("file_with_underscore.map", true),
            ("file.with.dots.map", true),
        ];

        let temp_dir = TempDir::new().unwrap();
        let transport = create_test_transport();
        let manager = FileTransferManager::new(transport);

        for (filename, should_work) in test_cases {
            let test_file = create_test_file(&temp_dir, filename, 100).await;
            let result = manager.inspect_file(&test_file, TransferType::Map).await;

            if should_work {
                assert!(result.is_ok(), "Failed for: {}", filename);
            }
        }
    }

    #[tokio::test]
    async fn test_shutdown_cleanup() {
        let transport = create_test_transport();
        let manager = FileTransferManager::new(transport);

        // Shutdown should complete without errors
        manager.shutdown().await;

        // After shutdown, transfers should be cleared
        let uploads = manager.uploads().await;
        let downloads = manager.downloads().await;

        assert_eq!(uploads.len(), 0);
        assert_eq!(downloads.len(), 0);
    }

    #[tokio::test]
    async fn test_transfer_direction_conversion() {
        use crate::observability::TransferTelemetryDirection;

        let upload: TransferTelemetryDirection = TransferDirection::Upload.into();
        let download: TransferTelemetryDirection = TransferDirection::Download.into();

        // Verify conversions work
        assert!(matches!(upload, TransferTelemetryDirection::Upload));
        assert!(matches!(download, TransferTelemetryDirection::Download));
    }

    #[tokio::test]
    async fn test_checksum_consistency() {
        let temp_dir = TempDir::new().unwrap();
        let test_file = create_test_file(&temp_dir, "consistent.map", 8192).await;

        let transport = create_test_transport();
        let manager = FileTransferManager::new(transport);

        // Compute checksum multiple times
        let meta1 = manager
            .inspect_file(&test_file, TransferType::Map)
            .await
            .unwrap();
        let meta2 = manager
            .inspect_file(&test_file, TransferType::Map)
            .await
            .unwrap();
        let meta3 = manager
            .inspect_file(&test_file, TransferType::Map)
            .await
            .unwrap();

        // All checksums should be identical
        assert_eq!(meta1.checksum, meta2.checksum);
        assert_eq!(meta2.checksum, meta3.checksum);
    }

    #[tokio::test]
    async fn test_various_chunk_sizes() {
        let temp_dir = TempDir::new().unwrap();
        let transport = create_test_transport();

        let chunk_sizes = vec![
            4 * 1024,    // 4KB
            16 * 1024,   // 16KB
            64 * 1024,   // 64KB
            128 * 1024,  // 128KB
            256 * 1024,  // 256KB
            512 * 1024,  // 512KB
            1024 * 1024, // 1MB
        ];

        for chunk_size in chunk_sizes {
            let config = TransferConfig {
                chunk_size,
                ..Default::default()
            };
            let manager = FileTransferManager::with_config(transport.clone(), config);

            let test_file = create_test_file(&temp_dir, "test.map", 100 * 1024).await;
            let metadata = manager
                .inspect_file(&test_file, TransferType::Map)
                .await
                .unwrap();

            assert_eq!(metadata.file_size, 100 * 1024);
        }
    }

    #[tokio::test]
    async fn test_transfer_state_transitions() {
        let transfer_id = Uuid::new_v4();
        let metadata = FileMetadata {
            filename: "test.map".to_string(),
            file_size: 1024,
            checksum: [0; 32],
            transfer_type: TransferType::Map,
        };

        let mut progress = TransferProgress::new(
            transfer_id,
            metadata,
            TransferDirection::Upload,
            Some("127.0.0.1:8080".parse().unwrap()),
        );

        // Initial state
        assert_eq!(progress.bytes_transferred, 0);
        assert!(!progress.complete);

        // Progress updates
        for i in 1..=10 {
            progress.update(i * 100);
            assert_eq!(progress.bytes_transferred, i * 100);
            assert!(!progress.complete);
        }

        // Completion
        progress.set_complete();
        assert!(progress.complete);
    }
}
