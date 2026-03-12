// FILE: xfer_save.rs
// Author: Ported from C++ (Colin Day, February 2002)
// Desc: Xfer hard disk write implementation

use super::xfer::*;
use std::fs::File;
use std::io::{Seek, SeekFrom, Write};

const MAX_XFER_STRING_LENGTH: usize = 255;

// ------------------------------------------------------------------------------------------------
// XferBlockData - Stack entry for tracking block positions
// ------------------------------------------------------------------------------------------------
struct XferBlockData {
    file_pos: XferFilePos,
    next: Option<Box<XferBlockData>>,
}

impl XferBlockData {
    fn new(file_pos: XferFilePos) -> Self {
        Self {
            file_pos,
            next: None,
        }
    }
}

// ------------------------------------------------------------------------------------------------
// XferSave - File writing implementation
// ------------------------------------------------------------------------------------------------
pub struct XferSave {
    base: XferBase,
    file: Option<File>,
    block_stack: Option<Box<XferBlockData>>,
}

impl XferSave {
    /// Create a new XferSave instance
    pub fn new() -> Self {
        Self {
            base: XferBase::new(XferMode::Save),
            file: None,
            block_stack: None,
        }
    }
}

impl Drop for XferSave {
    fn drop(&mut self) {
        // Warn if file was left open
        if self.file.is_some() {
            eprintln!(
                "Warning: Xfer file '{}' was left open",
                self.base.identifier
            );
            let _ = self.close();
        }

        // Block stack should be empty
        if self.block_stack.is_some() {
            eprintln!("Warning: XferSave::drop - block_stack was not None!");
            // Clean up the block stack
            while self.block_stack.is_some() {
                let current = self.block_stack.take();
                if let Some(mut node) = current {
                    self.block_stack = node.next.take();
                }
            }
        }
    }
}

impl Xfer for XferSave {
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

    /// Open file for writing
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

        // Open the file for writing (binary mode)
        match File::create(&identifier) {
            Ok(file) => {
                self.file = Some(file);
                Ok(())
            }
            Err(_) => {
                eprintln!("File '{}' could not be created", identifier);
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

        Ok(())
    }

    /// Write a placeholder at the current location and store this location.
    /// The next end_block will back up to this position and write the actual size.
    fn begin_block(&mut self) -> Result<XferBlockSize, XferStatus> {
        let file = self.file.as_mut().ok_or(XferStatus::FileNotOpen)?;

        // Get current file position
        let file_pos = file.stream_position().map_err(|_| XferStatus::WriteError)? as XferFilePos;

        // Write a placeholder
        let block_size: XferBlockSize = 0;
        let bytes = block_size.to_le_bytes();

        if file.write_all(&bytes).is_err() {
            eprintln!(
                "XferSave::begin_block - Error writing block size in '{}'",
                self.base.identifier
            );
            return Err(XferStatus::WriteError);
        }

        // Push this block position onto the stack
        let mut new_block = Box::new(XferBlockData::new(file_pos));
        new_block.next = self.block_stack.take();
        self.block_stack = Some(new_block);

        Ok(0)
    }

    /// Back up to the last begin block, write the file difference, and restore position
    fn end_block(&mut self) -> Result<(), XferStatus> {
        let file = self.file.as_mut().ok_or(XferStatus::FileNotOpen)?;

        // Make sure we have a block started
        let top = self.block_stack.take().ok_or_else(|| {
            eprintln!("Xfer end block called, but no matching begin block was found");
            XferStatus::BeginEndMismatch
        })?;

        // Save current file position
        let current_file_pos =
            file.stream_position().map_err(|_| XferStatus::WriteError)? as XferFilePos;

        // Pop the block descriptor off the stack
        self.block_stack = top.next;

        // Rewind to the block position
        file.seek(SeekFrom::Start(top.file_pos as u64))
            .map_err(|_| XferStatus::WriteError)?;

        // Calculate block size (excluding the size field itself)
        let block_size =
            (current_file_pos - top.file_pos - std::mem::size_of::<XferBlockSize>() as XferFilePos)
                as XferBlockSize;

        // Write the actual size
        let bytes = block_size.to_le_bytes();
        if file.write_all(&bytes).is_err() {
            eprintln!(
                "Error writing block size to file '{}'",
                self.base.identifier
            );
            return Err(XferStatus::WriteError);
        }

        // Return file pointer to current position
        file.seek(SeekFrom::Start(current_file_pos as u64))
            .map_err(|_| XferStatus::WriteError)?;

        Ok(())
    }

    /// Skip forward in the file (no-op for save)
    fn skip(&mut self, data_size: i32) -> Result<(), XferStatus> {
        let file = self.file.as_mut().ok_or(XferStatus::FileNotOpen)?;

        // Skip forward dataSize bytes
        file.seek(SeekFrom::Current(data_size as i64))
            .map_err(|_| XferStatus::WriteError)?;

        Ok(())
    }

    /// Entry point for xfering a snapshot
    fn xfer_snapshot(&mut self, snapshot: &mut dyn Snapshot) -> Result<(), XferStatus> {
        snapshot.xfer(self)
    }

    /// Save ASCII string
    fn xfer_ascii_string(&mut self, ascii_string_data: &mut String) -> Result<(), XferStatus> {
        // Sanity check length
        if ascii_string_data.len() > MAX_XFER_STRING_LENGTH {
            eprintln!("XferSave cannot save this ASCII string because it's too long");
            return Err(XferStatus::StringError);
        }

        // Save length of string to follow
        let len = ascii_string_data.len() as u8;
        self.xfer_unsigned_byte(&mut len.clone())?;

        // Save string data
        if len > 0 {
            let bytes = ascii_string_data.as_bytes();
            // SAFETY: bytes is a valid slice
            unsafe { self.xfer_user(bytes.as_ptr() as *mut u8, bytes.len())? };
        }

        Ok(())
    }

    /// Save Unicode string
    fn xfer_unicode_string(&mut self, unicode_string_data: &mut String) -> Result<(), XferStatus> {
        // Sanity check length
        if unicode_string_data.len() > MAX_XFER_STRING_LENGTH {
            eprintln!("XferSave cannot save this unicode string because it's too long");
            return Err(XferStatus::StringError);
        }

        // Convert to UTF-16 (wide char)
        let utf16: Vec<u16> = unicode_string_data.encode_utf16().collect();

        // Save length of string to follow
        let len = utf16.len() as u8;
        self.xfer_unsigned_byte(&mut len.clone())?;

        // Save string data
        if len > 0 {
            // Write as raw bytes (u16 array)
            let byte_slice = unsafe {
                std::slice::from_raw_parts(
                    utf16.as_ptr() as *const u8,
                    utf16.len() * std::mem::size_of::<u16>(),
                )
            };
            // SAFETY: byte_slice is a valid slice
            unsafe { self.xfer_user(byte_slice.as_ptr() as *mut u8, byte_slice.len())? };
        }

        Ok(())
    }

    /// Perform the write operation
    ///
    /// # Safety
    /// The caller must ensure that `data` points to a valid buffer
    /// of at least `data_size` bytes.
    unsafe fn xfer_implementation(
        &mut self,
        data: *mut u8,
        data_size: usize,
    ) -> Result<(), XferStatus> {
        let file = self.file.as_mut().ok_or(XferStatus::FileNotOpen)?;

        // Convert pointer to slice
        let slice = std::slice::from_raw_parts(data, data_size);

        // Write data to file
        if file.write_all(slice).is_err() {
            eprintln!(
                "XferSave - Error writing to file '{}'",
                self.base.identifier
            );
            return Err(XferStatus::WriteError);
        }

        Ok(())
    }
}

impl Default for XferSave {
    fn default() -> Self {
        Self::new()
    }
}
