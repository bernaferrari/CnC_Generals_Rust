// FILE: xfer_load.rs //////////////////////////////////////////////////////////
// Loading-specific data transfer functionality with CRC verification
///////////////////////////////////////////////////////////////////////////////

use super::snapshot::Snapshot;
use super::xfer::{Xfer, XferBlockSize, XferMode, XferStatus, XferVersion};
use super::xfer_crc::{CorruptionEntry, XferDeepCRC};
use std::collections::HashMap;
use std::io::{self, Cursor, Read, Seek, SeekFrom};

/// Load operation with versioning and CRC verification
pub struct XferLoad<R: Read> {
    reader: R,
    version: u32,
    bytes_read: u64,
    identifier: String,
    options: u32,
}

impl<R: Read> XferLoad<R> {
    pub fn new(reader: R, version: u32) -> Self {
        Self {
            reader,
            version,
            bytes_read: 0,
            identifier: String::new(),
            options: 0,
        }
    }

    pub fn version(&self) -> u32 {
        self.version
    }

    pub fn bytes_read(&self) -> u64 {
        self.bytes_read
    }
}

/// Load operation with deep CRC verification and corruption recovery
pub struct XferLoadWithCRC<R: Read + Seek> {
    reader: R,
    version: u32,
    crc_verifier: Option<XferDeepCRC<XferLoad<Cursor<Vec<u8>>>>>,
    checkpoint_map: HashMap<String, LoadCheckpoint>,
    corruption_recovery_enabled: bool,
}

/// Checkpoint for recovery
#[derive(Debug, Clone)]
pub struct LoadCheckpoint {
    pub position: u64,
    pub object_path: String,
    pub expected_crc: u32,
}

impl<R: Read + Seek> XferLoadWithCRC<R> {
    pub fn new(reader: R, version: u32) -> Self {
        Self {
            reader,
            version,
            crc_verifier: None,
            checkpoint_map: HashMap::new(),
            corruption_recovery_enabled: true,
        }
    }

    pub fn enable_crc_verification(&mut self) {
        let load_xfer = XferLoad::new(Cursor::new(Vec::new()), self.version);
        self.crc_verifier = Some(XferDeepCRC::new(load_xfer));
    }

    pub fn set_corruption_recovery(&mut self, enabled: bool) {
        self.corruption_recovery_enabled = enabled;
    }

    /// Create checkpoint for potential rollback
    pub fn create_checkpoint(&mut self, object_path: &str) -> io::Result<()> {
        let position = self.reader.stream_position()?;
        let checkpoint = LoadCheckpoint {
            position,
            object_path: object_path.to_string(),
            expected_crc: 0, // Will be filled in later
        };
        self.checkpoint_map
            .insert(object_path.to_string(), checkpoint);
        Ok(())
    }

    /// Rollback to checkpoint on corruption
    pub fn rollback_to_checkpoint(&mut self, object_path: &str) -> io::Result<()> {
        if let Some(checkpoint) = self.checkpoint_map.get(object_path) {
            self.reader.seek(SeekFrom::Start(checkpoint.position))?;
            Ok(())
        } else {
            Err(io::Error::new(
                io::ErrorKind::NotFound,
                format!("No checkpoint found for {}", object_path),
            ))
        }
    }

    /// Verify loaded data CRC
    pub fn verify_crc(&self, expected_crc: u32) -> Result<(), CorruptionEntry> {
        if let Some(verifier) = &self.crc_verifier {
            let actual_crc = verifier.get_global_crc();
            if actual_crc != expected_crc {
                return Err(CorruptionEntry {
                    object_path: "global".to_string(),
                    expected_crc,
                    actual_crc,
                    field_name: None,
                });
            }
        }
        Ok(())
    }

    /// Get corruption log
    pub fn get_corruption_log(&self) -> Vec<CorruptionEntry> {
        if let Some(verifier) = &self.crc_verifier {
            verifier.get_corruption_log().to_vec()
        } else {
            Vec::new()
        }
    }

    /// Check if corruption was detected
    pub fn has_corruption(&self) -> bool {
        if let Some(verifier) = &self.crc_verifier {
            verifier.has_corruption()
        } else {
            false
        }
    }

    /// Attempt to recover from corruption by skipping corrupted object
    pub fn attempt_recovery(&mut self, corrupted_path: &str) -> io::Result<bool> {
        if !self.corruption_recovery_enabled {
            return Ok(false);
        }

        // Try to find next valid checkpoint
        if self.rollback_to_checkpoint(corrupted_path).is_ok() {
            log::warn!("Rolled back to checkpoint: {}", corrupted_path);
            return Ok(true);
        }

        log::error!("Cannot recover from corruption at: {}", corrupted_path);
        Ok(false)
    }
}

impl<R: Read> Xfer for XferLoad<R> {
    fn get_xfer_mode(&self) -> XferMode {
        XferMode::Load
    }

    fn get_identifier(&self) -> &str {
        &self.identifier
    }

    fn set_options(&mut self, options: u32) {
        self.options |= options;
    }

    fn clear_options(&mut self, options: u32) {
        self.options &= !options;
    }

    fn get_options(&self) -> u32 {
        self.options
    }

    fn open(&mut self, identifier: &str) -> Result<(), XferStatus> {
        self.identifier = identifier.to_string();
        Ok(())
    }

    fn close(&mut self) -> Result<(), XferStatus> {
        Ok(())
    }

    fn begin_block(&mut self) -> Result<XferBlockSize, XferStatus> {
        // Read block size for load operations
        let mut size = 0i32;
        self.xfer_int(&mut size)
            .map_err(|_| XferStatus::ReadError)?;
        Ok(size)
    }

    fn end_block(&mut self) -> Result<(), XferStatus> {
        Ok(())
    }

    fn skip(&mut self, data_size: i32) -> Result<(), XferStatus> {
        if data_size <= 0 {
            return Ok(());
        }

        let mut buffer = vec![0u8; data_size as usize];
        self.reader
            .read_exact(&mut buffer)
            .map_err(|_| XferStatus::SkipError)?;
        Ok(())
    }

    fn xfer_snapshot(&mut self, _snapshot: &mut Snapshot) -> Result<(), XferStatus> {
        match Snapshot::load_from_reader(&mut self.reader) {
            Ok(loaded) => {
                *_snapshot = loaded;
                Ok(())
            }
            Err(err) => {
                log::error!("Failed to load snapshot: {}", err);
                Err(XferStatus::ReadError)
            }
        }
    }

    fn xfer_ascii_string(&mut self, ascii_string_data: &mut String) -> io::Result<()> {
        // C++ uses UnsignedByte (u8) for string length, max 255 chars
        // Matches C++ XferLoad.cpp lines 142-158
        let mut len = 0u8;
        self.xfer_unsigned_byte(&mut len)?;
        let mut bytes = vec![0u8; len as usize];
        if len > 0 {
            self.reader.read_exact(&mut bytes)?;
        }
        *ascii_string_data = String::from_utf8(bytes)
            .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "Invalid UTF-8"))?;
        Ok(())
    }

    fn xfer_unicode_string(&mut self, unicode_string_data: &mut String) -> io::Result<()> {
        // For now, treat unicode same as ASCII (UTF-8)
        self.xfer_ascii_string(unicode_string_data)
    }

    /// # Safety
    /// The caller must ensure that `data` points to a valid mutable buffer
    /// of at least `data_size` bytes.
    unsafe fn xfer_implementation(&mut self, data: *mut u8, data_size: usize) -> io::Result<()> {
        let slice = std::slice::from_raw_parts_mut(data, data_size);
        self.reader.read_exact(slice)?;
        self.bytes_read += data_size as u64;
        Ok(())
    }
}
