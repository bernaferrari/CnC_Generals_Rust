//! Command serialization and deserialization
//!
//! This module provides efficient serialization/deserialization for network commands
//! using multiple formats (binary, JSON, MessagePack) with compression support.

use crate::commands::{CommandPayload, NetCommand};
use crate::error::{NetworkError, NetworkResult};
use serde::{Deserialize, Serialize};
use std::io::{Read, Write};
use tracing::{debug, trace};

/// Serialization format
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SerializationFormat {
    /// Binary format using bincode (fastest, smallest)
    Binary,
    /// JSON format (human readable, larger)
    Json,
    /// MessagePack format (balanced)
    MessagePack,
}

impl Default for SerializationFormat {
    fn default() -> Self {
        Self::Binary
    }
}

/// Compression algorithm
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompressionAlgorithm {
    /// No compression
    None,
    /// Zlib compression (good balance)
    Zlib,
    /// LZ4 compression (fastest)
    Lz4,
    /// Zstd compression (best ratio)
    Zstd,
}

impl Default for CompressionAlgorithm {
    fn default() -> Self {
        Self::Zlib
    }
}

/// Serialization configuration
#[derive(Debug, Clone)]
pub struct SerializationConfig {
    /// Serialization format
    pub format: SerializationFormat,
    /// Compression algorithm
    pub compression: CompressionAlgorithm,
    /// Compression threshold (bytes)
    pub compression_threshold: usize,
    /// Enable type validation
    pub validate_types: bool,
    /// Enable size limits
    pub enforce_size_limits: bool,
    /// Maximum serialized size
    pub max_size: usize,
}

impl Default for SerializationConfig {
    fn default() -> Self {
        Self {
            format: SerializationFormat::Binary,
            compression: CompressionAlgorithm::Zlib,
            compression_threshold: 256,
            validate_types: true,
            enforce_size_limits: true,
            max_size: 64 * 1024, // 64KB
        }
    }
}

/// Command serializer/deserializer
pub struct CommandSerializer {
    config: SerializationConfig,
}

impl CommandSerializer {
    /// Create new serializer with default configuration
    pub fn new() -> Self {
        Self::with_config(SerializationConfig::default())
    }

    /// Create serializer with custom configuration
    pub fn with_config(config: SerializationConfig) -> Self {
        Self { config }
    }

    /// Serialize a command to bytes
    pub fn serialize(&self, command: &NetCommand) -> NetworkResult<Vec<u8>> {
        // Validate command if enabled
        if self.config.validate_types {
            self.validate_command(command)?;
        }

        // Serialize to intermediate format
        let intermediate = self.serialize_to_format(command)?;

        // Apply compression if warranted
        let final_data = if intermediate.len() > self.config.compression_threshold {
            self.compress_data(&intermediate)?
        } else {
            intermediate
        };

        // Check size limits
        if self.config.enforce_size_limits && final_data.len() > self.config.max_size {
            return Err(NetworkError::invalid_command(format!(
                "serialized command too large: {} bytes (max: {})",
                final_data.len(),
                self.config.max_size
            )));
        }

        trace!(
            "Serialized command {} ({} bytes)",
            command.id,
            final_data.len()
        );
        Ok(final_data)
    }

    /// Deserialize bytes to a command
    pub fn deserialize(&self, data: &[u8]) -> NetworkResult<NetCommand> {
        if data.is_empty() {
            return Err(NetworkError::invalid_packet("empty command data"));
        }

        // Check size limits
        if self.config.enforce_size_limits && data.len() > self.config.max_size {
            return Err(NetworkError::invalid_packet(format!(
                "command data too large: {} bytes (max: {})",
                data.len(),
                self.config.max_size
            )));
        }

        // Try to decompress (this will detect if data was compressed)
        let decompressed = self.decompress_data(data)?;

        // Deserialize from format
        let command = self.deserialize_from_format(&decompressed)?;

        // Validate if enabled
        if self.config.validate_types {
            self.validate_command(&command)?;
        }

        trace!("Deserialized command {} ({} bytes)", command.id, data.len());
        Ok(command)
    }

    /// Serialize to the configured format
    fn serialize_to_format(&self, command: &NetCommand) -> NetworkResult<Vec<u8>> {
        match self.config.format {
            SerializationFormat::Binary => bincode::serialize(command).map_err(|e| {
                NetworkError::serialization(format!("bincode serialization failed: {}", e))
            }),
            SerializationFormat::Json => serde_json::to_vec(command).map_err(|e| {
                NetworkError::serialization(format!("JSON serialization failed: {}", e))
            }),
            SerializationFormat::MessagePack => rmp_serde::to_vec(command).map_err(|e| {
                NetworkError::serialization(format!("MessagePack serialization failed: {}", e))
            }),
        }
    }

    /// Deserialize from the configured format
    fn deserialize_from_format(&self, data: &[u8]) -> NetworkResult<NetCommand> {
        match self.config.format {
            SerializationFormat::Binary => bincode::deserialize(data).map_err(|e| {
                NetworkError::serialization(format!("bincode deserialization failed: {}", e))
            }),
            SerializationFormat::Json => serde_json::from_slice(data).map_err(|e| {
                NetworkError::serialization(format!("JSON deserialization failed: {}", e))
            }),
            SerializationFormat::MessagePack => rmp_serde::from_slice(data).map_err(|e| {
                NetworkError::serialization(format!("MessagePack deserialization failed: {}", e))
            }),
        }
    }

    /// Compress data using the configured algorithm
    fn compress_data(&self, data: &[u8]) -> NetworkResult<Vec<u8>> {
        match self.config.compression {
            CompressionAlgorithm::None => Ok(data.to_vec()),
            CompressionAlgorithm::Zlib => {
                use flate2::write::ZlibEncoder;
                use flate2::Compression;

                let mut encoder = ZlibEncoder::new(Vec::new(), Compression::default());
                encoder.write_all(data).map_err(|e| {
                    NetworkError::compression(format!("zlib compression failed: {}", e))
                })?;
                encoder
                    .finish()
                    .map_err(|e| NetworkError::compression(format!("zlib finish failed: {}", e)))
            }
            CompressionAlgorithm::Lz4 => {
                #[cfg(feature = "compression")]
                {
                    Ok(lz4_flex::compress_prepend_size(data))
                }
                #[cfg(not(feature = "compression"))]
                {
                    warn!("LZ4 compression requested but feature not enabled");
                    Ok(data.to_vec())
                }
            }
            CompressionAlgorithm::Zstd => {
                #[cfg(feature = "compression")]
                {
                    zstd::bulk::compress(data, 3).map_err(|e| {
                        NetworkError::compression(format!("Zstd compression failed: {}", e))
                    })
                }
                #[cfg(not(feature = "compression"))]
                {
                    warn!("Zstd compression requested but feature not enabled");
                    Ok(data.to_vec())
                }
            }
        }
    }

    /// Decompress data, auto-detecting compression
    fn decompress_data(&self, data: &[u8]) -> NetworkResult<Vec<u8>> {
        // Try to detect compression type and decompress
        // In a real implementation, we'd store compression metadata in the header

        match self.config.compression {
            CompressionAlgorithm::None => Ok(data.to_vec()),
            CompressionAlgorithm::Zlib => {
                // Try zlib decompression, fall back to uncompressed if it fails
                use flate2::read::ZlibDecoder;

                let mut decoder = ZlibDecoder::new(data);
                let mut decompressed = Vec::new();

                match decoder.read_to_end(&mut decompressed) {
                    Ok(_) => Ok(decompressed),
                    Err(_) => {
                        // Probably not compressed, return original
                        debug!("Data not zlib compressed, using as-is");
                        Ok(data.to_vec())
                    }
                }
            }
            CompressionAlgorithm::Lz4 => {
                #[cfg(feature = "compression")]
                {
                    match lz4_flex::decompress_size_prepended(data) {
                        Ok(decompressed) => Ok(decompressed),
                        Err(_) => {
                            debug!("Data not LZ4 compressed, using as-is");
                            Ok(data.to_vec())
                        }
                    }
                }
                #[cfg(not(feature = "compression"))]
                Ok(data.to_vec())
            }
            CompressionAlgorithm::Zstd => {
                #[cfg(feature = "compression")]
                {
                    match zstd::bulk::decompress(data, self.config.max_size) {
                        Ok(decompressed) => Ok(decompressed),
                        Err(_) => {
                            debug!("Data not Zstd compressed, using as-is");
                            Ok(data.to_vec())
                        }
                    }
                }
                #[cfg(not(feature = "compression"))]
                Ok(data.to_vec())
            }
        }
    }

    /// Validate command structure
    fn validate_command(&self, command: &NetCommand) -> NetworkResult<()> {
        // Basic structure validation
        if command.player_id >= crate::config::MAX_PLAYERS {
            return Err(NetworkError::invalid_command("invalid player ID"));
        }

        // Payload validation
        match &command.payload {
            CommandPayload::GameCommand(game_data) => {
                if game_data.parameters.len() > 32 {
                    return Err(NetworkError::invalid_command(
                        "too many game command parameters",
                    ));
                }
            }
            CommandPayload::Chat(chat_data) => {
                if chat_data.message.len() > 512 {
                    return Err(NetworkError::invalid_command("chat message too long"));
                }
            }
            CommandPayload::Generic(data) => {
                if data.len() > 4096 {
                    return Err(NetworkError::invalid_command("generic payload too large"));
                }
            }
            _ => {} // Other payloads are fine
        }

        Ok(())
    }

    /// Get serialization statistics
    pub fn get_stats(&self, command: &NetCommand) -> NetworkResult<SerializationStats> {
        let original = self.serialize_to_format(command)?;
        let compressed = self.compress_data(&original)?;

        Ok(SerializationStats {
            original_size: original.len(),
            compressed_size: compressed.len(),
            compression_ratio: if original.len() > 0 {
                compressed.len() as f64 / original.len() as f64
            } else {
                1.0
            },
            format: self.config.format,
            compression: self.config.compression,
        })
    }
}

impl Default for CommandSerializer {
    fn default() -> Self {
        Self::new()
    }
}

/// Serialization statistics
#[derive(Debug, Clone)]
pub struct SerializationStats {
    /// Original size before compression
    pub original_size: usize,
    /// Size after compression
    pub compressed_size: usize,
    /// Compression ratio (compressed / original)
    pub compression_ratio: f64,
    /// Serialization format used
    pub format: SerializationFormat,
    /// Compression algorithm used
    pub compression: CompressionAlgorithm,
}

impl SerializationStats {
    /// Get space savings percentage
    pub fn space_savings_percent(&self) -> f64 {
        (1.0 - self.compression_ratio) * 100.0
    }
}

/// Batch serializer for multiple commands
pub struct BatchSerializer {
    serializer: CommandSerializer,
}

impl BatchSerializer {
    /// Create new batch serializer
    pub fn new() -> Self {
        Self {
            serializer: CommandSerializer::new(),
        }
    }

    /// Create with custom config
    pub fn with_config(config: SerializationConfig) -> Self {
        Self {
            serializer: CommandSerializer::with_config(config),
        }
    }

    /// Serialize multiple commands to a single buffer
    pub fn serialize_batch(&self, commands: &[NetCommand]) -> NetworkResult<Vec<u8>> {
        if commands.is_empty() {
            return Ok(Vec::new());
        }

        // Create a batch container
        let batch = CommandBatch {
            commands: commands.to_vec(),
            count: commands.len() as u32,
            checksum: self.calculate_batch_checksum(commands),
        };

        // Serialize the entire batch
        match self.serializer.config.format {
            SerializationFormat::Binary => bincode::serialize(&batch).map_err(|e| {
                NetworkError::serialization(format!("batch serialization failed: {}", e))
            }),
            SerializationFormat::Json => serde_json::to_vec(&batch).map_err(|e| {
                NetworkError::serialization(format!("batch serialization failed: {}", e))
            }),
            SerializationFormat::MessagePack => rmp_serde::to_vec(&batch).map_err(|e| {
                NetworkError::serialization(format!("batch serialization failed: {}", e))
            }),
        }
    }

    /// Deserialize multiple commands from a single buffer
    pub fn deserialize_batch(&self, data: &[u8]) -> NetworkResult<Vec<NetCommand>> {
        if data.is_empty() {
            return Ok(Vec::new());
        }

        // Deserialize the batch
        let batch: CommandBatch = match self.serializer.config.format {
            SerializationFormat::Binary => bincode::deserialize(data).map_err(|e| {
                NetworkError::serialization(format!("batch deserialization failed: {}", e))
            })?,
            SerializationFormat::Json => serde_json::from_slice(data).map_err(|e| {
                NetworkError::serialization(format!("batch deserialization failed: {}", e))
            })?,
            SerializationFormat::MessagePack => rmp_serde::from_slice(data).map_err(|e| {
                NetworkError::serialization(format!("batch deserialization failed: {}", e))
            })?,
        };

        // Validate batch
        if batch.commands.len() != batch.count as usize {
            return Err(NetworkError::invalid_packet("batch count mismatch"));
        }

        let expected_checksum = self.calculate_batch_checksum(&batch.commands);
        if batch.checksum != expected_checksum {
            return Err(NetworkError::invalid_packet("batch checksum mismatch"));
        }

        Ok(batch.commands)
    }

    /// Calculate checksum for command batch
    fn calculate_batch_checksum(&self, commands: &[NetCommand]) -> u32 {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};

        let mut hasher = DefaultHasher::new();
        for command in commands {
            command.id.hash(&mut hasher);
            command.command_type.hash(&mut hasher);
            command.player_id.hash(&mut hasher);
        }
        hasher.finish() as u32
    }
}

/// Container for batched commands
#[derive(Debug, Clone, Serialize, Deserialize)]
struct CommandBatch {
    commands: Vec<NetCommand>,
    count: u32,
    checksum: u32,
}

impl Default for BatchSerializer {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commands::NetCommand;
    use std::collections::HashMap;

    #[test]
    fn test_binary_serialization() {
        let serializer = CommandSerializer::new();
        let command = NetCommand::keep_alive(0);

        let serialized = serializer.serialize(&command).unwrap();
        let deserialized = serializer.deserialize(&serialized).unwrap();

        assert_eq!(command.id, deserialized.id);
        assert_eq!(command.command_type, deserialized.command_type);
        assert_eq!(command.player_id, deserialized.player_id);
    }

    #[test]
    fn test_json_serialization() {
        let mut config = SerializationConfig::default();
        config.format = SerializationFormat::Json;
        config.compression = CompressionAlgorithm::None;

        let serializer = CommandSerializer::with_config(config);
        let command = NetCommand::chat(0, "Hello!".to_string(), 0xFF);

        let serialized = serializer.serialize(&command).unwrap();
        let deserialized = serializer.deserialize(&serialized).unwrap();

        assert_eq!(command.id, deserialized.id);
        assert_eq!(command.command_type, deserialized.command_type);
    }

    #[test]
    fn test_messagepack_serialization() {
        let mut config = SerializationConfig::default();
        config.format = SerializationFormat::MessagePack;
        config.compression = CompressionAlgorithm::None;

        let serializer = CommandSerializer::with_config(config);
        let game_data = crate::commands::GameCommandData {
            command_type: 1,
            target_id: Some(123),
            position: Some((1.0, 2.0, 3.0)),
            parameters: HashMap::new(),
            checksum: 0,
        };

        let command = NetCommand::game_command(0, 100, game_data);
        let serialized = serializer.serialize(&command).unwrap();
        let deserialized = serializer.deserialize(&serialized).unwrap();

        assert_eq!(command.id, deserialized.id);
        assert_eq!(command.execution_frame, deserialized.execution_frame);
    }

    #[test]
    fn test_compression() {
        let mut config = SerializationConfig::default();
        config.compression = CompressionAlgorithm::Zlib;
        config.compression_threshold = 0; // Always compress

        let serializer = CommandSerializer::with_config(config);
        let command = NetCommand::chat(0, "A".repeat(200), 0xFF);

        let serialized = serializer.serialize(&command).unwrap();
        let deserialized = serializer.deserialize(&serialized).unwrap();

        assert_eq!(command.id, deserialized.id);

        // Check that compression actually happened
        let stats = serializer.get_stats(&command).unwrap();
        assert!(stats.compression_ratio < 1.0);
    }

    #[test]
    fn test_batch_serialization() {
        let batch_serializer = BatchSerializer::new();
        let commands = vec![
            NetCommand::keep_alive(0),
            NetCommand::chat(1, "Test".to_string(), 0xFF),
            NetCommand::keep_alive(2),
        ];

        let serialized = batch_serializer.serialize_batch(&commands).unwrap();
        let deserialized = batch_serializer.deserialize_batch(&serialized).unwrap();

        assert_eq!(commands.len(), deserialized.len());
        for (original, deserialized) in commands.iter().zip(deserialized.iter()) {
            assert_eq!(original.id, deserialized.id);
            assert_eq!(original.command_type, deserialized.command_type);
        }
    }

    #[test]
    fn test_size_limits() {
        let mut config = SerializationConfig::default();
        config.max_size = 100; // Very small limit

        let serializer = CommandSerializer::with_config(config);
        let large_command = NetCommand::chat(0, "x".repeat(1000), 0xFF);

        assert!(serializer.serialize(&large_command).is_err());
    }

    #[test]
    fn test_serialization_stats() {
        let serializer = CommandSerializer::new();
        let command = NetCommand::chat(0, "Hello World!".repeat(10), 0xFF);

        let stats = serializer.get_stats(&command).unwrap();
        assert!(stats.original_size > 0);
        assert!(stats.compressed_size > 0);
        assert!(stats.compression_ratio > 0.0);
    }
}
