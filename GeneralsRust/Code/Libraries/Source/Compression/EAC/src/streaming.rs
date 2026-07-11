//! Streaming compression and decompression for large files
//!
//! This module provides efficient streaming interfaces for processing large files
//! without loading them entirely into memory. Features include:
//! - Memory-mapped file I/O for optimal performance
//! - Real-time progress tracking
//! - Cancellable operations
//! - Async support with Tokio

use crate::decoder::{Decoder, DecoderConfig};
use crate::encoder::{Encoder, EncoderConfig};
use crate::{CompressionType, EacError, Result};
use memmap2::{Mmap, MmapOptions};
use rayon::prelude::*;
use std::fs::File;
use std::io::{BufReader, BufWriter, Read, Write};
use std::path::Path;

#[cfg(feature = "streaming")]
use tokio::fs::File as AsyncFile;

/// Progress callback for streaming operations
pub type ProgressCallback = Box<dyn Fn(f64) + Send + Sync>;

/// Streaming compression configuration
#[derive(Debug, Clone)]
pub struct StreamingConfig {
    /// Buffer size for I/O operations
    pub buffer_size: usize,
    /// Compression chunk size
    pub chunk_size: usize,
    /// Enable memory mapping for large files
    pub use_mmap: bool,
    /// Memory mapping threshold (use mmap if file > threshold)
    pub mmap_threshold: usize,
    /// Enable parallel processing
    pub parallel: bool,
    /// Maximum number of parallel workers
    pub max_workers: usize,
}

impl Default for StreamingConfig {
    fn default() -> Self {
        Self {
            buffer_size: 64 * 1024,  // 64KB buffer
            chunk_size: 1024 * 1024, // 1MB chunks
            use_mmap: true,
            mmap_threshold: 16 * 1024 * 1024, // 16MB threshold
            parallel: true,
            max_workers: rayon::current_num_threads(),
        }
    }
}

/// Streaming compressor for large files
pub struct StreamingCompressor {
    encoder: Encoder,
    config: StreamingConfig,
    compression_type: CompressionType,
    progress_callback: Option<ProgressCallback>,
}

impl StreamingCompressor {
    /// Create new streaming compressor
    pub fn new(compression_type: CompressionType) -> Self {
        Self::with_configs(
            compression_type,
            EncoderConfig::default(),
            StreamingConfig::default(),
        )
    }

    /// Create with custom configurations
    pub fn with_configs(
        compression_type: CompressionType,
        encoder_config: EncoderConfig,
        streaming_config: StreamingConfig,
    ) -> Self {
        Self {
            encoder: Encoder::with_config(encoder_config),
            config: streaming_config,
            compression_type,
            progress_callback: None,
        }
    }

    /// Set progress callback
    pub fn with_progress<F>(mut self, callback: F) -> Self
    where
        F: Fn(f64) + Send + Sync + 'static,
    {
        self.progress_callback = Some(Box::new(callback));
        self
    }

    /// Compress file to another file
    pub fn compress_file<P1, P2>(
        &mut self,
        input_path: P1,
        output_path: P2,
    ) -> Result<CompressionStats>
    where
        P1: AsRef<Path>,
        P2: AsRef<Path>,
    {
        let input_path = input_path.as_ref();
        let output_path = output_path.as_ref();

        log::info!(
            "Compressing file: {} -> {}",
            input_path.display(),
            output_path.display()
        );

        let input_file = File::open(input_path)?;
        let input_size = input_file.metadata()?.len() as usize;

        let output_file = File::create(output_path)?;

        // Choose compression strategy based on file size
        let compressed_size = if self.config.use_mmap && input_size >= self.config.mmap_threshold {
            self.compress_with_mmap(input_file, output_file, input_size)?
        } else {
            self.compress_with_buffers(input_file, output_file, input_size)?
        };

        Ok(CompressionStats {
            original_size: input_size,
            compressed_size,
            compression_ratio: compressed_size as f64 / input_size as f64,
            space_saving: 1.0 - (compressed_size as f64 / input_size as f64),
        })
    }

    /// Compress using memory-mapped I/O for large files
    fn compress_with_mmap(
        &mut self,
        input_file: File,
        mut output_file: File,
        input_size: usize,
    ) -> Result<usize> {
        log::debug!("Using memory-mapped compression for {} bytes", input_size);

        // Memory-map the input file
        let mmap = unsafe {
            MmapOptions::new()
                .map(&input_file)
                .map_err(EacError::Io)?
        };

        let chunks_count = input_size.div_ceil(self.config.chunk_size);

        let total_compressed = if self.config.parallel && chunks_count > 1 {
            // Parallel compression using memory-mapped chunks
            self.compress_mmap_parallel(&mmap, &mut output_file, chunks_count)?
        } else {
            // Sequential compression
            self.compress_mmap_sequential(&mmap, &mut output_file)?
        };

        output_file.flush()?;

        Ok(total_compressed)
    }

    /// Parallel memory-mapped compression
    fn compress_mmap_parallel(
        &mut self,
        mmap: &Mmap,
        output_file: &mut File,
        chunks_count: usize,
    ) -> Result<usize> {
        log::debug!("Compressing {} chunks in parallel", chunks_count);

        // Process chunks in parallel
        let compressed_chunks: Result<Vec<_>> = (0..chunks_count)
            .into_par_iter()
            .map(|i| {
                let start = i * self.config.chunk_size;
                let end = std::cmp::min(start + self.config.chunk_size, mmap.len());
                let chunk = &mmap[start..end];

                let mut encoder = Encoder::with_config(EncoderConfig::default());
                let compressed = encoder.encode(chunk, self.compression_type)?;

                // Update progress
                if let Some(ref callback) = self.progress_callback {
                    let progress = (i + 1) as f64 / chunks_count as f64;
                    callback(progress);
                }

                Ok(compressed)
            })
            .collect();

        let compressed_chunks = compressed_chunks?;

        // Write chunks to output file with metadata
        let mut total_size = 0;

        // Write chunk count and size info
        output_file.write_all(&(chunks_count as u32).to_le_bytes())?;
        output_file.write_all(&(self.config.chunk_size as u32).to_le_bytes())?;
        total_size += 8;

        // Write each compressed chunk
        for chunk in compressed_chunks {
            output_file.write_all(&(chunk.len() as u32).to_le_bytes())?;
            output_file.write_all(&chunk)?;
            total_size += 4 + chunk.len();
        }

        Ok(total_size)
    }

    /// Sequential memory-mapped compression
    fn compress_mmap_sequential(&mut self, mmap: &Mmap, output_file: &mut File) -> Result<usize> {
        let compressed = self.encoder.encode(mmap, self.compression_type)?;
        output_file.write_all(&compressed)?;

        if let Some(ref callback) = self.progress_callback {
            callback(1.0);
        }

        Ok(compressed.len())
    }

    /// Compress using buffered I/O for smaller files
    fn compress_with_buffers(
        &mut self,
        input_file: File,
        output_file: File,
        input_size: usize,
    ) -> Result<usize> {
        log::debug!("Using buffered compression for {} bytes", input_size);

        let mut reader = BufReader::with_capacity(self.config.buffer_size, input_file);
        let mut writer = BufWriter::with_capacity(self.config.buffer_size, output_file);

        let mut buffer = vec![0u8; self.config.chunk_size];
        let mut total_compressed = 0;
        let mut processed = 0;

        loop {
            let bytes_read = reader.read(&mut buffer)?;

            if bytes_read == 0 {
                break;
            }

            let chunk = &buffer[..bytes_read];
            let compressed = self.encoder.encode(chunk, self.compression_type)?;

            // Write compressed chunk size and data
            writer.write_all(&(compressed.len() as u32).to_le_bytes())?;
            writer.write_all(&compressed)?;

            total_compressed += 4 + compressed.len();
            processed += bytes_read;

            // Update progress
            if let Some(ref callback) = self.progress_callback {
                let progress = processed as f64 / input_size as f64;
                callback(progress);
            }
        }

        writer.flush()?;
        Ok(total_compressed)
    }
}

/// Streaming decompressor for large files
pub struct StreamingDecompressor {
    decoder: Decoder,
    config: StreamingConfig,
    progress_callback: Option<ProgressCallback>,
}

impl StreamingDecompressor {
    /// Create new streaming decompressor
    pub fn new() -> Self {
        Self::with_configs(DecoderConfig::default(), StreamingConfig::default())
    }

    /// Create with custom configurations
    pub fn with_configs(decoder_config: DecoderConfig, streaming_config: StreamingConfig) -> Self {
        Self {
            decoder: Decoder::with_config(decoder_config),
            config: streaming_config,
            progress_callback: None,
        }
    }

    /// Set progress callback
    pub fn with_progress<F>(mut self, callback: F) -> Self
    where
        F: Fn(f64) + Send + Sync + 'static,
    {
        self.progress_callback = Some(Box::new(callback));
        self
    }

    /// Decompress file to another file
    pub fn decompress_file<P1, P2>(
        &mut self,
        input_path: P1,
        output_path: P2,
    ) -> Result<DecompressionStats>
    where
        P1: AsRef<Path>,
        P2: AsRef<Path>,
    {
        let input_path = input_path.as_ref();
        let output_path = output_path.as_ref();

        log::info!(
            "Decompressing file: {} -> {}",
            input_path.display(),
            output_path.display()
        );

        let input_file = File::open(input_path)?;
        let compressed_size = input_file.metadata()?.len() as usize;

        let output_file = File::create(output_path)?;

        // Choose decompression strategy
        let decompressed_size =
            if self.config.use_mmap && compressed_size >= self.config.mmap_threshold {
                self.decompress_with_mmap(input_file, output_file, compressed_size)?
            } else {
                self.decompress_with_buffers(input_file, output_file, compressed_size)?
            };

        Ok(DecompressionStats {
            compressed_size,
            decompressed_size,
            compression_ratio: compressed_size as f64 / decompressed_size as f64,
        })
    }

    /// Decompress using memory-mapped I/O
    fn decompress_with_mmap(
        &mut self,
        input_file: File,
        mut output_file: File,
        compressed_size: usize,
    ) -> Result<usize> {
        log::debug!(
            "Using memory-mapped decompression for {} bytes",
            compressed_size
        );

        let mmap = unsafe {
            MmapOptions::new()
                .map(&input_file)
                .map_err(EacError::Io)?
        };

        let decompressed = self.decoder.decode(&mmap)?;
        output_file.write_all(&decompressed)?;

        if let Some(ref callback) = self.progress_callback {
            callback(1.0);
        }

        Ok(decompressed.len())
    }

    /// Decompress using buffered I/O
    fn decompress_with_buffers(
        &mut self,
        input_file: File,
        output_file: File,
        compressed_size: usize,
    ) -> Result<usize> {
        log::debug!("Using buffered decompression for {} bytes", compressed_size);

        let mut reader = BufReader::with_capacity(self.config.buffer_size, input_file);
        let mut writer = BufWriter::with_capacity(self.config.buffer_size, output_file);

        let mut total_decompressed = 0;
        let mut processed = 0;

        loop {
            // Read chunk size
            let mut size_buffer = [0u8; 4];
            match reader.read_exact(&mut size_buffer) {
                Ok(()) => {}
                Err(e) if e.kind() == std::io::ErrorKind::UnexpectedEof => break,
                Err(e) => return Err(EacError::Io(e)),
            }

            let chunk_size = u32::from_le_bytes(size_buffer) as usize;

            // Read compressed chunk
            let mut chunk_buffer = vec![0u8; chunk_size];
            reader.read_exact(&mut chunk_buffer)?;

            // Decompress chunk
            let decompressed = self.decoder.decode(&chunk_buffer)?;
            writer.write_all(&decompressed)?;

            total_decompressed += decompressed.len();
            processed += 4 + chunk_size;

            // Update progress
            if let Some(ref callback) = self.progress_callback {
                let progress = processed as f64 / compressed_size as f64;
                callback(progress);
            }
        }

        writer.flush()?;
        Ok(total_decompressed)
    }
}

impl Default for StreamingDecompressor {
    fn default() -> Self {
        Self::new()
    }
}

/// Compression statistics
#[derive(Debug, Clone)]
pub struct CompressionStats {
    pub original_size: usize,
    pub compressed_size: usize,
    pub compression_ratio: f64,
    pub space_saving: f64,
}

/// Decompression statistics
#[derive(Debug, Clone)]
pub struct DecompressionStats {
    pub compressed_size: usize,
    pub decompressed_size: usize,
    pub compression_ratio: f64,
}

// Async streaming support (when tokio feature is enabled)
#[cfg(feature = "streaming")]
pub mod async_streaming {
    use super::*;
    use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};

    /// Async streaming compressor
    pub struct AsyncStreamingCompressor {
        compressor: StreamingCompressor,
    }

    impl AsyncStreamingCompressor {
        pub fn new(compression_type: CompressionType) -> Self {
            Self {
                compressor: StreamingCompressor::new(compression_type),
            }
        }

        /// Compress async reader to async writer
        pub async fn compress_async<R, W>(
            &mut self,
            reader: &mut R,
            writer: &mut W,
        ) -> Result<usize>
        where
            R: AsyncRead + Unpin,
            W: AsyncWrite + Unpin,
        {
            let mut buffer = vec![0u8; self.compressor.config.chunk_size];
            let mut total_compressed = 0;

            loop {
                let bytes_read = reader
                    .read(&mut buffer)
                    .await
                    .map_err(EacError::Io)?;

                if bytes_read == 0 {
                    break;
                }

                let chunk = &buffer[..bytes_read];
                let compressed = self
                    .compressor
                    .encoder
                    .encode(chunk, self.compressor.compression_type)?;

                writer
                    .write_all(&(compressed.len() as u32).to_le_bytes())
                    .await
                    .map_err(EacError::Io)?;
                writer
                    .write_all(&compressed)
                    .await
                    .map_err(EacError::Io)?;

                total_compressed += 4 + compressed.len();
            }

            writer.flush().await.map_err(EacError::Io)?;
            Ok(total_compressed)
        }

        /// Compress file asynchronously
        pub async fn compress_file_async<P1, P2>(
            &mut self,
            input_path: P1,
            output_path: P2,
        ) -> Result<CompressionStats>
        where
            P1: AsRef<Path>,
            P2: AsRef<Path>,
        {
            let mut input_file = AsyncFile::open(input_path)
                .await
                .map_err(EacError::Io)?;
            let mut output_file = AsyncFile::create(output_path)
                .await
                .map_err(EacError::Io)?;

            let original_size = input_file
                .metadata()
                .await
                .map_err(EacError::Io)?
                .len() as usize;

            let compressed_size = self
                .compress_async(&mut input_file, &mut output_file)
                .await?;

            Ok(CompressionStats {
                original_size,
                compressed_size,
                compression_ratio: compressed_size as f64 / original_size as f64,
                space_saving: 1.0 - (compressed_size as f64 / original_size as f64),
            })
        }
    }

    /// Async streaming decompressor
    pub struct AsyncStreamingDecompressor {
        decompressor: StreamingDecompressor,
    }

    impl AsyncStreamingDecompressor {
        pub fn new() -> Self {
            Self {
                decompressor: StreamingDecompressor::new(),
            }
        }

        /// Decompress async reader to async writer
        pub async fn decompress_async<R, W>(
            &mut self,
            reader: &mut R,
            writer: &mut W,
        ) -> Result<usize>
        where
            R: AsyncRead + Unpin,
            W: AsyncWrite + Unpin,
        {
            let mut total_decompressed = 0;

            loop {
                // Read chunk size
                let mut size_buffer = [0u8; 4];
                match reader.read_exact(&mut size_buffer).await {
                    Ok(_) => {}
                    Err(e) if e.kind() == std::io::ErrorKind::UnexpectedEof => break,
                    Err(e) => return Err(EacError::Io(e)),
                }

                let chunk_size = u32::from_le_bytes(size_buffer) as usize;

                // Read compressed chunk
                let mut chunk_buffer = vec![0u8; chunk_size];
                reader
                    .read_exact(&mut chunk_buffer)
                    .await
                    .map_err(EacError::Io)?;

                // Decompress chunk
                let decompressed = self.decompressor.decoder.decode(&chunk_buffer)?;
                writer
                    .write_all(&decompressed)
                    .await
                    .map_err(EacError::Io)?;

                total_decompressed += decompressed.len();
            }

            writer.flush().await.map_err(EacError::Io)?;
            Ok(total_decompressed)
        }
    }

    impl Default for AsyncStreamingDecompressor {
        fn default() -> Self {
            Self::new()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Cursor, Write};
    use tempfile::{tempdir, NamedTempFile};

    #[test]
    fn test_streaming_config() {
        let config = StreamingConfig::default();
        assert_eq!(config.buffer_size, 64 * 1024);
        assert_eq!(config.chunk_size, 1024 * 1024);
        assert!(config.use_mmap);
        assert!(config.parallel);
    }

    #[test]
    fn test_streaming_compressor_small_data() -> Result<()> {
        let temp_dir = tempdir().unwrap();
        let input_path = temp_dir.path().join("input.txt");
        let output_path = temp_dir.path().join("output.eac");

        // Create test file
        let test_data = b"Hello, World! This is a test for streaming compression.";
        std::fs::write(&input_path, test_data).unwrap();

        // Compress
        let mut compressor = StreamingCompressor::new(CompressionType::RefPack);
        let stats = compressor.compress_file(&input_path, &output_path)?;

        assert_eq!(stats.original_size, test_data.len());
        assert!(stats.compressed_size > 0);
        assert!(std::fs::metadata(&output_path).unwrap().len() > 0);

        Ok(())
    }

    #[test]
    fn test_streaming_compressor_with_progress() -> Result<()> {
        use std::sync::{Arc, Mutex};

        let temp_dir = tempdir().unwrap();
        let input_path = temp_dir.path().join("input.txt");
        let output_path = temp_dir.path().join("output.eac");

        // Create larger test file
        let test_data = vec![42u8; 10000];
        std::fs::write(&input_path, &test_data).unwrap();

        // Track progress
        let progress = Arc::new(Mutex::new(0.0));
        let progress_clone = Arc::clone(&progress);

        let mut compressor =
            StreamingCompressor::new(CompressionType::RefPack).with_progress(move |p| {
                *progress_clone.lock().unwrap() = p;
            });

        let stats = compressor.compress_file(&input_path, &output_path)?;

        let final_progress = *progress.lock().unwrap();
        assert_eq!(final_progress, 1.0);
        assert_eq!(stats.original_size, test_data.len());

        Ok(())
    }

    #[test]
    fn test_streaming_decompressor() -> Result<()> {
        let temp_dir = tempdir().unwrap();
        let original_path = temp_dir.path().join("original.txt");
        let compressed_path = temp_dir.path().join("compressed.eac");
        let decompressed_path = temp_dir.path().join("decompressed.txt");

        // Create test data
        let test_data = b"This is test data for streaming compression and decompression.";
        std::fs::write(&original_path, test_data).unwrap();

        // Compress
        let mut compressor = StreamingCompressor::new(CompressionType::RefPack);
        let compress_stats = compressor.compress_file(&original_path, &compressed_path)?;

        // Decompress
        let mut decompressor = StreamingDecompressor::new();
        let decompress_stats =
            decompressor.decompress_file(&compressed_path, &decompressed_path)?;

        // Verify
        let decompressed_data = std::fs::read(&decompressed_path).unwrap();
        assert_eq!(test_data, &decompressed_data[..]);
        assert_eq!(
            compress_stats.original_size,
            decompress_stats.decompressed_size
        );

        Ok(())
    }

    #[test]
    fn test_compression_stats() {
        let stats = CompressionStats {
            original_size: 1000,
            compressed_size: 600,
            compression_ratio: 0.6,
            space_saving: 0.4,
        };

        assert_eq!(stats.original_size, 1000);
        assert_eq!(stats.compressed_size, 600);
        assert_eq!(stats.compression_ratio, 0.6);
        assert_eq!(stats.space_saving, 0.4);
    }

    #[test]
    fn test_decompression_stats() {
        let stats = DecompressionStats {
            compressed_size: 600,
            decompressed_size: 1000,
            compression_ratio: 0.6,
        };

        assert_eq!(stats.compressed_size, 600);
        assert_eq!(stats.decompressed_size, 1000);
        assert_eq!(stats.compression_ratio, 0.6);
    }

    #[cfg(feature = "streaming")]
    #[tokio::test]
    async fn test_async_streaming_compressor() -> Result<()> {
        use async_streaming::AsyncStreamingCompressor;

        let mut compressor = AsyncStreamingCompressor::new(CompressionType::RefPack);

        let test_data = b"Async streaming compression test data.";
        let mut reader = Cursor::new(test_data);
        let mut writer = Vec::new();

        let compressed_size = compressor.compress_async(&mut reader, &mut writer).await?;

        assert!(compressed_size > 0);
        assert!(!writer.is_empty());

        Ok(())
    }
}
