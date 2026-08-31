// FILE: xfer_load.rs //////////////////////////////////////////////////////////
// Loading-specific data transfer functionality with CRC verification
///////////////////////////////////////////////////////////////////////////////

use super::snapshot::Snapshotable;
use super::xfer::{Xfer, XferBlockSize, XferMode, XferStatus};
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
    post_process_snapshot_callback: Option<Box<dyn FnMut()>>,
}

impl<R: Read> XferLoad<R> {
    pub fn new(reader: R, version: u32) -> Self {
        Self {
            reader,
            version,
            bytes_read: 0,
            identifier: String::new(),
            options: 0,
            post_process_snapshot_callback: None,
        }
    }

    /// C++ `XferLoad::xferSnapshot` registers via `TheGameState->addPostProcessSnapshot`
    /// (`XferLoad.cpp:164-166`) unless `XO_NO_POST_PROCESSING`.
    pub fn set_post_process_snapshot_callback(&mut self, callback: Option<Box<dyn FnMut()>>) {
        self.post_process_snapshot_callback = callback;
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

    fn xfer_snapshot(&mut self, snapshot: &mut dyn Snapshotable) -> Result<(), XferStatus> {
        // C++ XferLoad.cpp:161-166 — run snapshot->xfer(this), then register
        // post-process. Do not replace `*snapshot` from a self-describing blob.
        snapshot.xfer(self).map_err(|_| XferStatus::ReadError)?;
        if (self.options & super::xfer::XferOptions::NO_POST_PROCESSING) == 0 {
            if let Some(callback) = self.post_process_snapshot_callback.as_mut() {
                callback();
            }
        }
        Ok(())
    }

    fn xfer_ascii_string(&mut self, ascii_string_data: &mut String) -> io::Result<()> {
        // C++ XferLoad.cpp:142-158 — UnsignedByte length + raw 8-bit payload
        // into AsciiString::set. `String::from_utf8` rejects locale-encoded
        // names (Windows-1252 map/player strings). Map each byte to U+00xx
        // so the payload survives a Rust String the same way leftover does.
        let mut len = 0u8;
        self.xfer_unsigned_byte(&mut len)?;
        let mut bytes = vec![0u8; len as usize];
        if len > 0 {
            self.reader.read_exact(&mut bytes)?;
        }
        *ascii_string_data = bytes.iter().map(|&b| b as char).collect();
        Ok(())
    }

    fn xfer_unicode_string(&mut self, unicode_string_data: &mut String) -> io::Result<()> {
        // C++ XferLoad.cpp:168-190 — WideChar units into UnicodeString::set.
        // Unpaired surrogates are legal C++ payload; `from_utf16` aborts the
        // whole save. Match leftover `from_utf16_lossy`.
        let mut len = 0u8;
        self.xfer_unsigned_byte(&mut len)?;
        let mut units = Vec::with_capacity(len as usize);
        for _ in 0..len {
            let mut bytes = [0u8; 2];
            self.reader.read_exact(&mut bytes)?;
            units.push(u16::from_le_bytes(bytes));
        }
        *unicode_string_data = String::from_utf16_lossy(&units);
        Ok(())
    }

    /// # Safety
    /// The caller must ensure that `data` points to a valid mutable buffer
    /// of at least `data_size` bytes.
            // SAFETY: caller contract — data valid for writes of data_size;
            // read_exact fills exactly that many bytes from the stream.
    unsafe fn xfer_implementation(&mut self, data: *mut u8, data_size: usize) -> io::Result<()> {
        let slice = std::slice::from_raw_parts_mut(data, data_size);
        self.reader.read_exact(slice)?;
        self.bytes_read += data_size as u64;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::super::snapshot::Snapshot;
    use super::*;
    use std::io::Cursor;

    #[test]
    fn unicode_string_reads_cpp_wide_char_layout() {
        let bytes = vec![3, 0x41, 0x00, 0xdf, 0x00, 0x22, 0x6f];
        let mut xfer = XferLoad::new(Cursor::new(bytes), 1);
        let mut value = String::new();

        xfer.xfer_unicode_string(&mut value).unwrap();

        assert_eq!(value, "A\u{00df}\u{6f22}");
    }

    #[test]
    fn ascii_string_reads_cpp_8bit_locale_payload() {
        // C++ AsciiString::set keeps any 8-bit byte. 0xE9 is valid CP1252
        // "é" and invalid UTF-8 as a lone starter + 'x'.
        let bytes = vec![3, b'M', 0xE9, b'x'];
        let mut xfer = XferLoad::new(Cursor::new(bytes), 1);
        let mut value = String::new();

        xfer.xfer_ascii_string(&mut value).unwrap();

        assert_eq!(value, "M\u{00e9}x");
    }

    #[test]
    fn unicode_string_reads_unpaired_utf16_surrogate() {
        // Lone high surrogate U+D800. C++ UnicodeString::set accepts it;
        // leftover uses from_utf16_lossy (U+FFFD).
        let bytes = vec![1, 0x00, 0xD8];
        let mut xfer = XferLoad::new(Cursor::new(bytes), 1);
        let mut value = String::new();

        xfer.xfer_unicode_string(&mut value).unwrap();

        assert_eq!(value, "\u{FFFD}");
    }

    #[test]
    fn xfer_snapshot_visits_existing_object_instead_of_replacing() {
        // C++ XferLoad::xferSnapshot (XferLoad.cpp:150-167) calls
        // snapshot->xfer(this) then TheGameState->addPostProcessSnapshot
        // unless XO_NO_POST_PROCESSING. Pre-fix Rust replaced *snapshot via
        // Snapshot::load_from_reader (a different blob layout).
        use super::super::xfer::XferOptions;
        use super::super::xfer_save::XferSave;

        let mut saved = Snapshot::new(42, 3.5);
        saved.add_data(b"visit-me");
        saved.set_metadata("k".to_string(), "v".to_string());
        let mut encoded = Vec::new();
        {
            let mut save = XferSave::new(Cursor::new(&mut encoded), 1);
            saved.xfer(&mut save).expect("visitor save");
        }

        let mut loaded = Snapshot::new(0, 0.0);
        loaded.add_data(b"stale");
        let post_process_calls = std::sync::Arc::new(std::sync::Mutex::new(0u32));
        {
            let mut load = XferLoad::new(Cursor::new(&encoded), 1);
            let calls = std::sync::Arc::clone(&post_process_calls);
            load.set_post_process_snapshot_callback(Some(Box::new(move || {
                *calls.lock().expect("post-process lock") += 1;
            })));
            load.xfer_snapshot(&mut loaded).expect("visitor load");
        }
        assert_eq!(loaded.frame_number(), 42);
        assert_eq!(loaded.timestamp(), 3.5);
        assert_eq!(loaded.data(), b"visit-me");
        assert_eq!(loaded.get_metadata("k"), Some(&"v".to_string()));
        assert_eq!(*post_process_calls.lock().expect("post-process lock"), 1);

        let mut skipped = Snapshot::new(0, 0.0);
        let skipped_calls = std::sync::Arc::new(std::sync::Mutex::new(0u32));
        {
            let mut load = XferLoad::new(Cursor::new(&encoded), 1);
            load.set_options(XferOptions::NO_POST_PROCESSING);
            let calls = std::sync::Arc::clone(&skipped_calls);
            load.set_post_process_snapshot_callback(Some(Box::new(move || {
                *calls.lock().expect("skip lock") += 1;
            })));
            load.xfer_snapshot(&mut skipped).expect("no post-process");
        }
        assert_eq!(skipped.frame_number(), 42);
        assert_eq!(*skipped_calls.lock().expect("skip lock"), 0);
    }
}
