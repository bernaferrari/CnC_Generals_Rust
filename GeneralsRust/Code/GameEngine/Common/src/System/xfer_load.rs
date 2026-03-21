// FILE: xfer_load.rs
// Author: Ported from C++ (Colin Day, February 2002)
// Desc: Xfer implementation for loading from disk

use super::xfer::*;
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};

const MAX_XFER_LOAD_STRING_BUFFER: usize = 1024;

// ------------------------------------------------------------------------------------------------
// XferLoad - File reading implementation
// ------------------------------------------------------------------------------------------------
pub struct XferLoad {
    base: XferBase,
    file: Option<File>,
    post_process_snapshot_callback: Option<Box<dyn FnMut()>>,
}

impl XferLoad {
    /// Create a new XferLoad instance
    pub fn new() -> Self {
        Self {
            base: XferBase::new(XferMode::Load),
            file: None,
            post_process_snapshot_callback: None,
        }
    }

    /// Register a callback that records loaded snapshots for post-processing.
    pub fn set_post_process_snapshot_callback(
        &mut self,
        callback: Option<Box<dyn FnMut()>>,
    ) {
        self.post_process_snapshot_callback = callback;
    }
}

impl Drop for XferLoad {
    fn drop(&mut self) {
        // Warn if file was left open
        if self.file.is_some() {
            eprintln!(
                "Warning: Xfer file '{}' was left open",
                self.base.identifier
            );
            let _ = self.close();
        }
    }
}

impl Xfer for XferLoad {
    fn get_xfer_mode(&self) -> XferMode {
        self.base.xfer_mode
    }

    fn get_identifier(&self) -> &str {
        &self.base.identifier
    }

    fn set_options(&mut self, options: u32) {
        bit_set(&mut self.base.options, options);
    }

    fn clear_options(&mut self, options: u32) {
        bit_clear(&mut self.base.options, options);
    }

    fn get_options(&self) -> u32 {
        self.base.options
    }

    /// Open file for reading
    fn open(&mut self, identifier: String) -> Result<(), XferStatus> {
        // Check if already open
        if self.file.is_some() {
            eprintln!(
                "Cannot open file '{}' cause we've already got '{}' open",
                identifier, self.base.identifier
            );
            return Err(XferStatus::FileAlreadyOpen);
        }

        // Call base class
        self.base.open_base(identifier.clone());

        // Open the file for reading (binary mode)
        match File::open(&identifier) {
            Ok(file) => {
                self.file = Some(file);
                Ok(())
            }
            Err(_) => {
                eprintln!("File '{}' not found", identifier);
                Err(XferStatus::FileNotFound)
            }
        }
    }

    /// Close the current file
    fn close(&mut self) -> Result<(), XferStatus> {
        // Check if file is open
        if self.file.is_none() {
            eprintln!("Xfer close called, but no file was open");
            return Err(XferStatus::FileNotOpen);
        }

        // Close the file (drop does this)
        self.file = None;

        // Clear the filename
        self.base.identifier.clear();
        self.post_process_snapshot_callback = None;

        Ok(())
    }

    /// Read a block size descriptor from the file at the current position
    fn begin_block(&mut self) -> Result<XferBlockSize, XferStatus> {
        let file = self.file.as_mut().ok_or(XferStatus::FileNotOpen)?;

        // Read block size
        let mut bytes = [0u8; std::mem::size_of::<XferBlockSize>()];

        if file.read_exact(&mut bytes).is_err() {
            eprintln!(
                "Xfer - Error reading block size for '{}'",
                self.base.identifier
            );
            return Ok(0);
        }

        let block_size = XferBlockSize::from_le_bytes(bytes);
        Ok(block_size)
    }

    /// End block - this does nothing when reading
    fn end_block(&mut self) -> Result<(), XferStatus> {
        Ok(())
    }

    /// Skip forward in the file
    fn skip(&mut self, data_size: i32) -> Result<(), XferStatus> {
        let file = self.file.as_mut().ok_or(XferStatus::FileNotOpen)?;

        // Sanity check
        if data_size < 0 {
            eprintln!(
                "XferLoad::skip - dataSize '{}' must be greater than 0",
                data_size
            );
            return Err(XferStatus::InvalidParameters);
        }

        // Skip datasize bytes from the current position
        if file.seek(SeekFrom::Current(data_size as i64)).is_err() {
            return Err(XferStatus::SkipError);
        }

        Ok(())
    }

    /// Entry point for xfering a snapshot
    fn xfer_snapshot(&mut self, snapshot: &mut dyn Snapshot) -> Result<(), XferStatus> {
        // Run the xfer function of the snapshot
        snapshot.xfer(self)?;

        // Record the snapshot for deferred load fixups using the caller-provided hook.
        if !bit_test(self.get_options(), xfer_options::NO_POST_PROCESSING) {
            if let Some(callback) = self.post_process_snapshot_callback.as_mut() {
                callback();
            }
        }

        Ok(())
    }

    /// Read string from file and store in ASCII string
    fn xfer_ascii_string(&mut self, ascii_string_data: &mut String) -> Result<(), XferStatus> {
        // Read length of string to follow
        let mut len: u8 = 0;
        self.xfer_unsigned_byte(&mut len)?;

        // Read string data
        let mut buffer = vec![0u8; len as usize];

        if len > 0 {
            // SAFETY: buffer was allocated with len elements
            unsafe { self.xfer_user(buffer.as_mut_ptr(), len as usize)? };
        }

        // Convert to string
        *ascii_string_data = String::from_utf8_lossy(&buffer[..len as usize]).into_owned();

        Ok(())
    }

    /// Read string from file and store in Unicode string
    fn xfer_unicode_string(&mut self, unicode_string_data: &mut String) -> Result<(), XferStatus> {
        // Read length of string to follow
        let mut len: u8 = 0;
        self.xfer_unsigned_byte(&mut len)?;

        // Read string data as UTF-16
        let mut buffer = vec![0u16; len as usize];

        if len > 0 {
            // Read as raw bytes
            let byte_len = len as usize * std::mem::size_of::<u16>();
            let byte_buffer =
                unsafe { std::slice::from_raw_parts_mut(buffer.as_mut_ptr() as *mut u8, byte_len) };
            // SAFETY: byte_buffer is valid for byte_len bytes
            unsafe { self.xfer_user(byte_buffer.as_mut_ptr(), byte_len)? };
        }

        // Convert UTF-16 to String
        *unicode_string_data = String::from_utf16_lossy(&buffer[..len as usize]);

        Ok(())
    }

    /// Perform the read operation
    /// # Safety
    /// The caller must ensure that `data` points to a valid mutable buffer
    /// of at least `data_size` bytes.
    unsafe fn xfer_implementation(
        &mut self,
        data: *mut u8,
        data_size: usize,
    ) -> Result<(), XferStatus> {
        let file = self.file.as_mut().ok_or(XferStatus::FileNotOpen)?;

        // Convert pointer to mutable slice
        let slice = std::slice::from_raw_parts_mut(data, data_size);

        // Read data from file
        if file.read_exact(slice).is_err() {
            eprintln!(
                "XferLoad - Error reading from file '{}'",
                self.base.identifier
            );
            return Err(XferStatus::ReadError);
        }

        Ok(())
    }
}

impl Default for XferLoad {
    fn default() -> Self {
        Self::new()
    }
}
