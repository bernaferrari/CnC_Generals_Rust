//! Raw File I/O Implementation
//!
//! This module provides a Rust implementation of the RawFileClass from the original
//! Command & Conquer Generals WWLib library. It provides low-level file I/O operations
//! with support for file biasing, cross-platform compatibility, and robust error handling.
//!
//! # Examples
//!
//! ```
//! use wwlib_rust::rawfile::{RawFile, FileRights};
//! use std::io::Result;
//!
//! fn example_usage() -> Result<()> {
//!     // Create a new file object
//!     let mut file = RawFile::new("test.txt");
//!     
//!     // Open for reading
//!     file.open(FileRights::READ)?;
//!     
//!     // Read some data
//!     let mut buffer = vec![0u8; 1024];
//!     let bytes_read = file.read(&mut buffer)?;
//!     
//!     // Close the file
//!     file.close()?;
//!     
//!     Ok(())
//! }
//! ```

use std::fs::{File, OpenOptions};
use std::io::{self, BufReader, BufWriter, Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::time::SystemTime;

/// File access rights flags
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FileRights(u32);

impl FileRights {
    /// Read access
    pub const READ: FileRights = FileRights(1);
    /// Write access
    pub const WRITE: FileRights = FileRights(2);
    /// Read and Write access
    pub const READ_WRITE: FileRights = FileRights(3);

    /// Check if read access is enabled
    pub fn can_read(self) -> bool {
        (self.0 & Self::READ.0) != 0
    }

    /// Check if write access is enabled
    pub fn can_write(self) -> bool {
        (self.0 & Self::WRITE.0) != 0
    }

    /// Combine with another rights flag
    pub fn combine(self, other: FileRights) -> FileRights {
        FileRights(self.0 | other.0)
    }
}

/// Seek origin for file positioning operations
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SeekOrigin {
    /// Seek from start of file
    Start,
    /// Seek from current position
    Current,
    /// Seek from end of file
    End,
}

impl From<SeekOrigin> for SeekFrom {
    fn from(origin: SeekOrigin) -> Self {
        match origin {
            SeekOrigin::Start => SeekFrom::Start(0),
            SeekOrigin::Current => SeekFrom::Current(0),
            SeekOrigin::End => SeekFrom::End(0),
        }
    }
}

/// File handle wrapper that supports both buffered and unbuffered operations
enum FileHandle {
    /// Buffered reader for read-only access
    Reader(BufReader<File>),
    /// Buffered writer for write-only access
    Writer(BufWriter<File>),
    /// Direct file handle for read-write access
    ReadWrite(File),
    /// No file handle (closed state)
    Closed,
}

impl FileHandle {
    /// Check if the file handle is open
    fn is_open(&self) -> bool {
        !matches!(self, FileHandle::Closed)
    }

    /// Get mutable reference to the underlying file for seeking operations
    fn as_file_mut(&mut self) -> io::Result<&mut File> {
        match self {
            FileHandle::Reader(reader) => Ok(reader.get_mut()),
            FileHandle::Writer(writer) => Ok(writer.get_mut()),
            FileHandle::ReadWrite(file) => Ok(file),
            FileHandle::Closed => Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "File is not open",
            )),
        }
    }

    /// Close the file handle
    fn close(&mut self) -> io::Result<()> {
        match std::mem::replace(self, FileHandle::Closed) {
            FileHandle::Writer(mut writer) => writer.flush(),
            _ => Ok(()),
        }
    }

    /// Flush any buffered writes
    fn flush(&mut self) -> io::Result<()> {
        match self {
            FileHandle::Writer(writer) => writer.flush(),
            FileHandle::ReadWrite(file) => file.flush(),
            _ => Ok(()),
        }
    }
}

/// Raw File I/O class providing low-level file operations
///
/// This struct provides a Rust implementation of the original RawFileClass,
/// supporting file biasing, cross-platform operations, and robust error handling.
pub struct RawFile {
    /// The file handle wrapper
    handle: FileHandle,
    /// The filename associated with this file object
    filename: PathBuf,
    /// The access rights used to open the file
    rights: FileRights,
    /// Bias start position (for sub-file views)
    bias_start: u64,
    /// Bias length (for sub-file views), None means no bias
    bias_length: Option<u64>,
}

impl RawFile {
    /// Create a new RawFile instance with no filename
    ///
    /// # Examples
    ///
    /// ```
    /// use wwlib_rust::rawfile::RawFile;
    ///
    /// let file = RawFile::default();
    /// ```
    pub fn new() -> Self {
        Self::default()
    }

    /// Create a new RawFile instance with the specified filename
    ///
    /// # Arguments
    ///
    /// * `filename` - The path to the file
    ///
    /// # Examples
    ///
    /// ```
    /// use wwlib_rust::rawfile::RawFile;
    ///
    /// let file = RawFile::with_name("test.txt");
    /// ```
    pub fn with_name<P: AsRef<Path>>(filename: P) -> Self {
        Self {
            handle: FileHandle::Closed,
            filename: filename.as_ref().to_path_buf(),
            rights: FileRights::READ,
            bias_start: 0,
            bias_length: None,
        }
    }

    /// Get the filename associated with this file object
    ///
    /// # Returns
    ///
    /// The filename as a string slice, or None if no filename is set
    ///
    /// # Examples
    ///
    /// ```
    /// use wwlib_rust::rawfile::RawFile;
    ///
    /// let file = RawFile::with_name("test.txt");
    /// assert_eq!(file.filename(), Some("test.txt"));
    /// ```
    pub fn filename(&self) -> Option<&str> {
        self.filename.to_str()
    }

    /// Set the filename for this file object
    ///
    /// # Arguments
    ///
    /// * `filename` - The new filename
    ///
    /// # Returns
    ///
    /// The filename that was set
    ///
    /// # Examples
    ///
    /// ```
    /// use wwlib_rust::rawfile::RawFile;
    ///
    /// let mut file = RawFile::new();
    /// file.set_name("test.txt");
    /// assert_eq!(file.filename(), Some("test.txt"));
    /// ```
    pub fn set_name<P: AsRef<Path>>(&mut self, filename: P) -> &str {
        // Clear any existing bias when setting a new name
        self.bias(0, None);
        self.filename = filename.as_ref().to_path_buf();
        self.filename.to_str().unwrap_or("")
    }

    /// Check if the file is currently open
    ///
    /// # Returns
    ///
    /// `true` if the file is open, `false` otherwise
    ///
    /// # Examples
    ///
    /// ```
    /// use wwlib_rust::rawfile::{RawFile, FileRights};
    ///
    /// let mut file = RawFile::with_name("test.txt");
    /// assert!(!file.is_open());
    ///
    /// file.open(FileRights::READ).ok();
    /// assert!(file.is_open());
    /// ```
    pub fn is_open(&self) -> bool {
        self.handle.is_open()
    }

    /// Check if the file is available for opening
    ///
    /// # Arguments
    ///
    /// * `forced` - If true, will attempt to open and close the file to verify availability
    ///
    /// # Returns
    ///
    /// `true` if the file is available, `false` otherwise
    ///
    /// # Examples
    ///
    /// ```
    /// use wwlib_rust::rawfile::RawFile;
    ///
    /// let file = RawFile::with_name("nonexistent.txt");
    /// assert!(!file.is_available(false));
    /// ```
    pub fn is_available(&self, forced: bool) -> bool {
        if self.filename.as_os_str().is_empty() {
            return false;
        }

        // If file is already open, it must be available
        if self.is_open() {
            return true;
        }

        if forced {
            // Try to open and close the file
            let mut temp_file = RawFile::with_name(&self.filename);
            if temp_file.open(FileRights::READ).is_ok() {
                temp_file.close().ok();
                return true;
            }
            return false;
        }

        // Simple existence check
        self.filename.exists()
    }

    /// Open the file with specified access rights
    ///
    /// # Arguments
    ///
    /// * `rights` - The access rights to use when opening the file
    ///
    /// # Returns
    ///
    /// `Ok(())` if successful, or an `io::Error` if the operation failed
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use wwlib_rust::rawfile::{RawFile, FileRights};
    ///
    /// let mut file = RawFile::with_name("test.txt");
    /// file.open(FileRights::READ)?;
    /// # Ok::<(), std::io::Error>(())
    /// ```
    pub fn open(&mut self, rights: FileRights) -> io::Result<()> {
        self.close()?;

        if self.filename.as_os_str().is_empty() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "No filename specified",
            ));
        }

        self.rights = rights;

        let file_handle = match rights {
            FileRights::READ => {
                let file = File::open(&self.filename)?;
                FileHandle::Reader(BufReader::new(file))
            }
            FileRights::WRITE => {
                let file = OpenOptions::new()
                    .write(true)
                    .create(true)
                    .truncate(true)
                    .open(&self.filename)?;
                FileHandle::Writer(BufWriter::new(file))
            }
            FileRights::READ_WRITE => {
                let file = OpenOptions::new()
                    .read(true)
                    .write(true)
                    .create(true)
                    .open(&self.filename)?;
                FileHandle::ReadWrite(file)
            }
            _ => {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "Invalid file access rights",
                ));
            }
        };

        self.handle = file_handle;

        // Position file for biased access if needed
        if self.bias_start > 0 || self.bias_length.is_some() {
            self.seek(0, SeekOrigin::Start)?;
        }

        Ok(())
    }

    /// Open the file with a filename and access rights in one operation
    ///
    /// # Arguments
    ///
    /// * `filename` - The filename to open
    /// * `rights` - The access rights to use
    ///
    /// # Returns
    ///
    /// `Ok(())` if successful, or an `io::Error` if the operation failed
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use wwlib_rust::rawfile::{RawFile, FileRights};
    ///
    /// let mut file = RawFile::new();
    /// file.open_with_name("test.txt", FileRights::READ)?;
    /// # Ok::<(), std::io::Error>(())
    /// ```
    pub fn open_with_name<P: AsRef<Path>>(
        &mut self,
        filename: P,
        rights: FileRights,
    ) -> io::Result<()> {
        self.set_name(filename);
        self.open(rights)
    }

    /// Close the file
    ///
    /// # Returns
    ///
    /// `Ok(())` if successful, or an `io::Error` if the operation failed
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use wwlib_rust::rawfile::{RawFile, FileRights};
    ///
    /// let mut file = RawFile::with_name("test.txt");
    /// file.open(FileRights::READ)?;
    /// file.close()?;
    /// # Ok::<(), std::io::Error>(())
    /// ```
    pub fn close(&mut self) -> io::Result<()> {
        self.handle.close()
    }

    /// Read data from the file into a buffer
    ///
    /// # Arguments
    ///
    /// * `buffer` - The buffer to read data into
    ///
    /// # Returns
    ///
    /// The number of bytes read, or an `io::Error` if the operation failed
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use wwlib_rust::rawfile::{RawFile, FileRights};
    ///
    /// let mut file = RawFile::with_name("test.txt");
    /// file.open(FileRights::READ)?;
    ///
    /// let mut buffer = vec![0u8; 1024];
    /// let bytes_read = file.read(&mut buffer)?;
    /// # Ok::<(), std::io::Error>(())
    /// ```
    pub fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        let mut opened_here = false;

        // Auto-open the file if not already open
        if !self.is_open() {
            self.open(FileRights::READ)?;
            opened_here = true;
        }

        let mut requested_size = buffer.len();

        // Limit read size based on bias
        if let Some(bias_length) = self.bias_length {
            let current_pos = self.tell()?;
            let remaining = bias_length.saturating_sub(current_pos);
            requested_size = requested_size.min(remaining as usize);
        }

        let bytes_read = match &mut self.handle {
            FileHandle::Reader(reader) => reader.read(&mut buffer[..requested_size])?,
            FileHandle::ReadWrite(file) => file.read(&mut buffer[..requested_size])?,
            FileHandle::Writer(_) => {
                return Err(io::Error::new(
                    io::ErrorKind::PermissionDenied,
                    "File opened for write-only access",
                ));
            }
            FileHandle::Closed => {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "File is not open",
                ));
            }
        };

        // Auto-close if we opened it
        if opened_here {
            self.close()?;
        }

        Ok(bytes_read)
    }

    /// Write data from a buffer to the file
    ///
    /// # Arguments
    ///
    /// * `buffer` - The buffer containing data to write
    ///
    /// # Returns
    ///
    /// The number of bytes written, or an `io::Error` if the operation failed
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use wwlib_rust::rawfile::{RawFile, FileRights};
    ///
    /// let mut file = RawFile::with_name("test.txt");
    /// file.open(FileRights::WRITE)?;
    ///
    /// let data = b"Hello, world!";
    /// let bytes_written = file.write(data)?;
    /// # Ok::<(), std::io::Error>(())
    /// ```
    pub fn write(&mut self, buffer: &[u8]) -> io::Result<usize> {
        let mut opened_here = false;

        // Auto-open the file if not already open
        if !self.is_open() {
            self.open(FileRights::WRITE)?;
            opened_here = true;
        }

        let bytes_written = match &mut self.handle {
            FileHandle::Writer(writer) => writer.write(buffer)?,
            FileHandle::ReadWrite(file) => file.write(buffer)?,
            FileHandle::Reader(_) => {
                return Err(io::Error::new(
                    io::ErrorKind::PermissionDenied,
                    "File opened for read-only access",
                ));
            }
            FileHandle::Closed => {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "File is not open",
                ));
            }
        };

        // Update bias length if necessary
        if self.bias_length.is_some() {
            let current_pos = self.raw_tell()?;
            let bias_start = self.bias_start;
            if let Some(ref mut bias_length) = self.bias_length {
                if current_pos > bias_start + *bias_length {
                    *bias_length = current_pos - bias_start;
                }
            }
        }

        // Auto-close if we opened it
        if opened_here {
            self.close()?;
        }

        Ok(bytes_written)
    }

    /// Seek to a position in the file
    ///
    /// # Arguments
    ///
    /// * `pos` - The position to seek to (relative to origin)
    /// * `origin` - The origin for the seek operation
    ///
    /// # Returns
    ///
    /// The new position from the start of the file, or an `io::Error` if the operation failed
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use wwlib_rust::rawfile::{RawFile, FileRights, SeekOrigin};
    ///
    /// let mut file = RawFile::with_name("test.txt");
    /// file.open(FileRights::READ)?;
    ///
    /// let new_pos = file.seek(100, SeekOrigin::Start)?;
    /// # Ok::<(), std::io::Error>(())
    /// ```
    pub fn seek(&mut self, pos: i64, origin: SeekOrigin) -> io::Result<u64> {
        if !self.is_open() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "File is not open",
            ));
        }

        // Handle biased file seeking
        if let Some(bias_length) = self.bias_length {
            let seek_pos = match origin {
                SeekOrigin::Start => {
                    let pos = pos.max(0) as u64;
                    let clamped_pos = pos.min(bias_length);
                    self.bias_start + clamped_pos
                }
                SeekOrigin::Current => {
                    // For current, we perform the raw seek and then adjust
                    let raw_pos = match &mut self.handle {
                        FileHandle::Reader(reader) => reader.seek(SeekFrom::Current(pos))?,
                        FileHandle::Writer(writer) => writer.seek(SeekFrom::Current(pos))?,
                        FileHandle::ReadWrite(file) => file.seek(SeekFrom::Current(pos))?,
                        FileHandle::Closed => {
                            return Err(io::Error::new(
                                io::ErrorKind::InvalidInput,
                                "File is not open",
                            ))
                        }
                    };
                    let biased_pos = raw_pos.saturating_sub(self.bias_start);

                    // Clamp to bias bounds
                    if biased_pos > bias_length {
                        let _final_pos = match &mut self.handle {
                            FileHandle::Reader(reader) => {
                                reader.seek(SeekFrom::Start(self.bias_start + bias_length))?
                            }
                            FileHandle::Writer(writer) => {
                                writer.seek(SeekFrom::Start(self.bias_start + bias_length))?
                            }
                            FileHandle::ReadWrite(file) => {
                                file.seek(SeekFrom::Start(self.bias_start + bias_length))?
                            }
                            FileHandle::Closed => {
                                return Err(io::Error::new(
                                    io::ErrorKind::InvalidInput,
                                    "File is not open",
                                ))
                            }
                        };
                        return Ok(bias_length);
                    }
                    return Ok(biased_pos);
                }
                SeekOrigin::End => self.bias_start + bias_length + (pos as u64),
            };

            let final_pos = match &mut self.handle {
                FileHandle::Reader(reader) => reader.seek(SeekFrom::Start(seek_pos))?,
                FileHandle::Writer(writer) => writer.seek(SeekFrom::Start(seek_pos))?,
                FileHandle::ReadWrite(file) => file.seek(SeekFrom::Start(seek_pos))?,
                FileHandle::Closed => {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidInput,
                        "File is not open",
                    ))
                }
            };
            Ok(final_pos.saturating_sub(self.bias_start))
        } else {
            // Unbiased seek
            let seek_from = match origin {
                SeekOrigin::Start => SeekFrom::Start(pos.max(0) as u64),
                SeekOrigin::Current => SeekFrom::Current(pos),
                SeekOrigin::End => SeekFrom::End(pos),
            };

            match &mut self.handle {
                FileHandle::Reader(reader) => reader.seek(seek_from),
                FileHandle::Writer(writer) => writer.seek(seek_from),
                FileHandle::ReadWrite(file) => file.seek(seek_from),
                FileHandle::Closed => Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "File is not open",
                )),
            }
        }
    }

    /// Get the current position in the file
    ///
    /// # Returns
    ///
    /// The current position from the start of the (possibly biased) file
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use wwlib_rust::rawfile::{RawFile, FileRights};
    ///
    /// let mut file = RawFile::with_name("test.txt");
    /// file.open(FileRights::READ)?;
    ///
    /// let pos = file.tell()?;
    /// # Ok::<(), std::io::Error>(())
    /// ```
    pub fn tell(&mut self) -> io::Result<u64> {
        if !self.is_open() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "File is not open",
            ));
        }

        let raw_pos = match &mut self.handle {
            FileHandle::Reader(reader) => {
                // For BufReader, we need to account for buffering
                // The stream_position method gives us the logical position
                reader.stream_position()?
            }
            FileHandle::Writer(writer) => writer.stream_position()?,
            FileHandle::ReadWrite(file) => file.seek(SeekFrom::Current(0))?,
            FileHandle::Closed => {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "File is not open",
                ));
            }
        };

        // Apply bias if present
        if self.bias_length.is_some() {
            Ok(raw_pos.saturating_sub(self.bias_start))
        } else {
            Ok(raw_pos)
        }
    }

    /// Get the size of the file
    ///
    /// # Returns
    ///
    /// The size of the (possibly biased) file in bytes
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use wwlib_rust::rawfile::RawFile;
    ///
    /// let mut file = RawFile::with_name("test.txt");
    /// let size = file.size()?;
    /// # Ok::<(), std::io::Error>(())
    /// ```
    pub fn size(&mut self) -> io::Result<u64> {
        // If biased, return the bias length
        if let Some(bias_length) = self.bias_length {
            return Ok(bias_length);
        }

        let mut opened_here = false;

        if !self.is_open() {
            self.open(FileRights::READ)?;
            opened_here = true;
        }

        let file = self.handle.as_file_mut()?;
        let metadata = file.metadata()?;
        let size = metadata.len();

        if opened_here {
            self.close()?;
        }

        // Set bias length for future reference
        self.bias_length = Some(size.saturating_sub(self.bias_start));

        Ok(self.bias_length.unwrap())
    }

    /// Create an empty file
    ///
    /// # Returns
    ///
    /// `Ok(true)` if successful, `Ok(false)` if failed, or an `io::Error`
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use wwlib_rust::rawfile::RawFile;
    ///
    /// let mut file = RawFile::with_name("new_file.txt");
    /// let created = file.create()?;
    /// # Ok::<(), std::io::Error>(())
    /// ```
    pub fn create(&mut self) -> io::Result<bool> {
        self.close()?;

        match self.open(FileRights::WRITE) {
            Ok(()) => {
                // If biased, seek to ensure correct file length
                if self.bias_length.is_some() {
                    self.seek(0, SeekOrigin::Start)?;
                }
                self.close()?;
                Ok(true)
            }
            Err(_) => Ok(false),
        }
    }

    /// Delete the file from disk
    ///
    /// # Returns
    ///
    /// `Ok(true)` if the file was deleted, `Ok(false)` if it didn't exist
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use wwlib_rust::rawfile::RawFile;
    ///
    /// let mut file = RawFile::with_name("file_to_delete.txt");
    /// let deleted = file.delete()?;
    /// # Ok::<(), std::io::Error>(())
    /// ```
    pub fn delete(&mut self) -> io::Result<bool> {
        self.close()?;

        if self.filename.as_os_str().is_empty() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "No filename specified",
            ));
        }

        if !self.is_available(false) {
            return Ok(false);
        }

        match std::fs::remove_file(&self.filename) {
            Ok(()) => Ok(true),
            Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(false),
            Err(e) => Err(e),
        }
    }

    /// Get the last modification time of the file
    ///
    /// # Returns
    ///
    /// The file's last modification time
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use wwlib_rust::rawfile::RawFile;
    ///
    /// let mut file = RawFile::with_name("test.txt");
    /// let mod_time = file.get_date_time()?;
    /// # Ok::<(), std::io::Error>(())
    /// ```
    pub fn get_date_time(&mut self) -> io::Result<SystemTime> {
        let mut opened_here = false;

        if !self.is_open() {
            self.open(FileRights::READ)?;
            opened_here = true;
        }

        let file = self.handle.as_file_mut()?;
        let metadata = file.metadata()?;
        let modified = metadata.modified()?;

        if opened_here {
            self.close()?;
        }

        Ok(modified)
    }

    /// Set file bias to create a sub-file view
    ///
    /// This allows treating a portion of a file as if it were the entire file.
    /// This is useful for files that contain other files (like archive formats).
    ///
    /// # Arguments
    ///
    /// * `start` - The starting offset in the file
    /// * `length` - The length of the sub-file view, or None to use the rest of the file
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use wwlib_rust::rawfile::RawFile;
    ///
    /// let mut file = RawFile::with_name("archive.dat");
    ///
    /// // Create a view of bytes 1000-2000 in the file
    /// file.bias(1000, Some(1000));
    ///
    /// // Now all operations work on this 1000-byte window
    /// # Ok::<(), std::io::Error>(())
    /// ```
    pub fn bias(&mut self, start: u64, length: Option<u64>) {
        if start == 0 {
            self.bias_start = 0;
            self.bias_length = None;
            return;
        }

        // Get current file size to calculate bias
        let current_size = if let Ok(size) = self.size() { size } else { 0 };

        self.bias_start += start;

        if let Some(length) = length {
            let available_length = current_size.saturating_sub(start);
            self.bias_length = Some(length.min(available_length));
        }

        // If file is open, seek to valid position
        if self.is_open() {
            let _ = self.seek(0, SeekOrigin::Start);
        }
    }

    /// Get the underlying file handle
    ///
    /// # Returns
    ///
    /// A reference to the underlying file handle, or None if not open
    pub fn get_file_handle(&mut self) -> Option<&mut File> {
        match &mut self.handle {
            FileHandle::Reader(reader) => Some(reader.get_mut()),
            FileHandle::Writer(writer) => Some(writer.get_mut()),
            FileHandle::ReadWrite(file) => Some(file),
            FileHandle::Closed => None,
        }
    }

    /// Flush any pending writes
    ///
    /// # Returns
    ///
    /// `Ok(())` if successful, or an `io::Error` if the operation failed
    pub fn flush(&mut self) -> io::Result<()> {
        self.handle.flush()
    }

    /// Perform an unbiased seek (ignoring any bias settings)
    fn raw_seek(&mut self, pos: i64, origin: SeekOrigin) -> io::Result<u64> {
        if !self.is_open() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "File is not open",
            ));
        }

        let seek_from = match origin {
            SeekOrigin::Start => SeekFrom::Start(pos.max(0) as u64),
            SeekOrigin::Current => SeekFrom::Current(pos),
            SeekOrigin::End => SeekFrom::End(pos),
        };

        let file = self.handle.as_file_mut()?;
        file.seek(seek_from)
    }

    /// Get the current unbiased position in the file
    fn raw_tell(&mut self) -> io::Result<u64> {
        self.raw_seek(0, SeekOrigin::Current)
    }
}

impl Default for RawFile {
    fn default() -> Self {
        Self {
            handle: FileHandle::Closed,
            filename: PathBuf::new(),
            rights: FileRights::READ,
            bias_start: 0,
            bias_length: None,
        }
    }
}

impl Drop for RawFile {
    /// Automatically close the file when the object is dropped
    fn drop(&mut self) {
        let _ = self.close();
    }
}

// Thread-safe implementation
unsafe impl Send for RawFile {}
unsafe impl Sync for RawFile {}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::io::Write as StdWrite;

    fn create_test_file(name: &str, content: &[u8]) -> std::io::Result<()> {
        let mut file = fs::File::create(name)?;
        file.write_all(content)?;
        Ok(())
    }

    fn cleanup_test_file(name: &str) {
        let _ = fs::remove_file(name);
    }

    #[test]
    fn test_file_creation_and_basic_operations() {
        let test_file = "test_basic.txt";
        let test_content = b"Hello, World!";

        // Clean up any existing test file
        cleanup_test_file(test_file);

        let mut file = RawFile::with_name(test_file);

        // Test filename retrieval
        assert_eq!(file.filename(), Some(test_file));

        // File should not be available initially
        assert!(!file.is_available(false));
        assert!(!file.is_open());

        // Create the file
        assert!(file.create().unwrap());

        // Write some content
        file.open(FileRights::WRITE).unwrap();
        let written = file.write(test_content).unwrap();
        assert_eq!(written, test_content.len());
        file.close().unwrap();

        // Read the content back
        file.open(FileRights::READ).unwrap();
        let mut buffer = vec![0u8; test_content.len()];
        let read = file.read(&mut buffer).unwrap();
        assert_eq!(read, test_content.len());
        assert_eq!(&buffer, test_content);

        // Test size
        let size = file.size().unwrap();
        assert_eq!(size, test_content.len() as u64);

        file.close().unwrap();

        // Test deletion
        assert!(file.delete().unwrap());
        assert!(!file.is_available(false));
    }

    #[test]
    fn test_file_seeking() {
        let test_file = "test_seeking.txt";
        let test_content = b"0123456789ABCDEFGHIJ";

        cleanup_test_file(test_file);
        create_test_file(test_file, test_content).unwrap();

        let mut file = RawFile::with_name(test_file);
        file.open(FileRights::READ).unwrap();

        // Test seek from start
        let pos = file.seek(5, SeekOrigin::Start).unwrap();
        assert_eq!(pos, 5);

        // Read a character to verify position
        let mut buffer = [0u8; 1];
        file.read(&mut buffer).unwrap();
        assert_eq!(buffer[0], b'5');

        // Test tell
        let current_pos = file.tell().unwrap();
        assert_eq!(current_pos, 6);

        // Test seek from current
        file.seek(-3, SeekOrigin::Current).unwrap();
        file.read(&mut buffer).unwrap();
        assert_eq!(buffer[0], b'3');

        // Test seek from end
        file.seek(-1, SeekOrigin::End).unwrap();
        file.read(&mut buffer).unwrap();
        assert_eq!(buffer[0], b'J');

        file.close().unwrap();
        cleanup_test_file(test_file);
    }

    #[test]
    fn test_file_bias() {
        let test_file = "test_bias.txt";
        let test_content = b"0123456789ABCDEFGHIJ";

        cleanup_test_file(test_file);
        create_test_file(test_file, test_content).unwrap();

        let mut file = RawFile::with_name(test_file);

        // Set bias to view only bytes 5-10
        file.bias(5, Some(5));
        file.open(FileRights::READ).unwrap();

        // Size should reflect the bias
        let size = file.size().unwrap();
        assert_eq!(size, 5);

        // Reading from the start should give us the biased content
        let mut buffer = [0u8; 5];
        let read = file.read(&mut buffer).unwrap();
        assert_eq!(read, 5);
        assert_eq!(&buffer, b"56789");

        // Seeking past the bias should be clamped
        let pos = file.seek(10, SeekOrigin::Start).unwrap();
        assert_eq!(pos, 5); // Should be clamped to bias length

        file.close().unwrap();
        cleanup_test_file(test_file);
    }

    #[test]
    fn test_auto_open_close() {
        let test_file = "test_auto.txt";
        let test_content = b"Auto open/close test";

        cleanup_test_file(test_file);
        create_test_file(test_file, test_content).unwrap();

        let mut file = RawFile::with_name(test_file);

        // Reading from a closed file should auto-open it
        let mut buffer = vec![0u8; test_content.len()];
        let read = file.read(&mut buffer).unwrap();
        assert_eq!(read, test_content.len());
        assert_eq!(&buffer, test_content);

        // File should be closed again after the read
        assert!(!file.is_open());

        cleanup_test_file(test_file);
    }

    #[test]
    fn test_file_rights() {
        let test_file = "test_rights.txt";

        cleanup_test_file(test_file);

        let _file = RawFile::with_name(test_file);

        // Test write rights
        assert!(FileRights::WRITE.can_write());
        assert!(!FileRights::WRITE.can_read());

        // Test read rights
        assert!(FileRights::READ.can_read());
        assert!(!FileRights::READ.can_write());

        // Test combined rights
        let combined = FileRights::READ.combine(FileRights::WRITE);
        assert!(combined.can_read());
        assert!(combined.can_write());
        assert_eq!(combined, FileRights::READ_WRITE);

        cleanup_test_file(test_file);
    }

    #[test]
    fn test_error_conditions() {
        let mut file = RawFile::new();

        // Opening without a filename should fail
        assert!(file.open(FileRights::READ).is_err());

        // Reading from a closed file with invalid name should fail
        file.set_name("nonexistent_file_12345.txt");
        let mut buffer = [0u8; 10];
        assert!(file.read(&mut buffer).is_err());
    }
}
