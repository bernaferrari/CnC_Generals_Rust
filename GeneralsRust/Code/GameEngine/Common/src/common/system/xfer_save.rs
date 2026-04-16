// FILE: xfer_save.rs //////////////////////////////////////////////////////////
// Saving-specific data transfer functionality
///////////////////////////////////////////////////////////////////////////////

use super::snapshot::Snapshot;
use super::xfer::{Xfer, XferBlockSize, XferMode, XferStatus};
use std::io::{self, Seek, SeekFrom, Write};

pub struct XferSave<W: Write + Seek> {
    writer: W,
    version: u32,
    identifier: String,
    options: u32,
    block_stack: Vec<u64>,
}

impl<W: Write + Seek> XferSave<W> {
    pub fn new(writer: W, version: u32) -> Self {
        Self {
            writer,
            version,
            identifier: String::new(),
            options: 0,
            block_stack: Vec::new(),
        }
    }

    pub fn version(&self) -> u32 {
        self.version
    }
}

impl<W: Write + Seek> Xfer for XferSave<W> {
    fn get_xfer_mode(&self) -> XferMode {
        XferMode::Save
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
        let block_pos = self
            .writer
            .stream_position()
            .map_err(|_| XferStatus::WriteError)?;
        let placeholder: XferBlockSize = 0;
        self.writer
            .write_all(&placeholder.to_le_bytes())
            .map_err(|_| XferStatus::WriteError)?;
        self.block_stack.push(block_pos);
        Ok(0)
    }

    fn end_block(&mut self) -> Result<(), XferStatus> {
        let start_pos = self.block_stack.pop().ok_or(XferStatus::BeginEndMismatch)?;
        let end_pos = self
            .writer
            .stream_position()
            .map_err(|_| XferStatus::WriteError)?;
        let payload_size = end_pos
            .checked_sub(start_pos + std::mem::size_of::<XferBlockSize>() as u64)
            .ok_or(XferStatus::WriteError)? as XferBlockSize;

        self.writer
            .seek(SeekFrom::Start(start_pos))
            .map_err(|_| XferStatus::WriteError)?;
        self.writer
            .write_all(&payload_size.to_le_bytes())
            .map_err(|_| XferStatus::WriteError)?;
        self.writer
            .seek(SeekFrom::Start(end_pos))
            .map_err(|_| XferStatus::WriteError)?;

        Ok(())
    }

    fn skip(&mut self, data_size: i32) -> Result<(), XferStatus> {
        if data_size < 0 {
            return Err(XferStatus::InvalidParameters);
        }
        self.writer
            .seek(SeekFrom::Current(data_size as i64))
            .map_err(|_| XferStatus::WriteError)?;
        Ok(())
    }

    fn xfer_snapshot(&mut self, _snapshot: &mut Snapshot) -> Result<(), XferStatus> {
        _snapshot
            .save_to_writer(&mut self.writer)
            .map_err(|_| XferStatus::WriteError)
    }

    fn xfer_ascii_string(&mut self, ascii_string_data: &mut String) -> io::Result<()> {
        let bytes = ascii_string_data.as_bytes();
        // C++ uses UnsignedByte (u8) for string length, max 255 chars
        // Matches C++ XferSave.cpp lines 219-232
        if bytes.len() > 255 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "XferSave cannot save this ascii string because it's too long (max 255)",
            ));
        }
        let mut len = bytes.len() as u8;
        self.xfer_unsigned_byte(&mut len)?;
        if len > 0 {
            self.writer.write_all(bytes)?;
        }
        Ok(())
    }

    fn xfer_unicode_string(&mut self, unicode_string_data: &mut String) -> io::Result<()> {
        // For now, treat unicode same as ASCII (UTF-8)
        self.xfer_ascii_string(unicode_string_data)
    }

    /// # Safety
    /// The caller must ensure that `data` points to a valid buffer
    /// of at least `data_size` bytes.
    unsafe fn xfer_implementation(&mut self, data: *mut u8, data_size: usize) -> io::Result<()> {
        let slice = std::slice::from_raw_parts(data, data_size);
        self.writer.write_all(slice)?;
        Ok(())
    }
}
