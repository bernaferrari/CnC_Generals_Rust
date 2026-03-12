use crate::rawfile::{FileRights, RawFile, SeekOrigin};
use std::io;
use std::path::Path;

pub struct BufferedFile {
    file: RawFile,
    buffer: Vec<u8>,
    buffer_available: usize,
    buffer_offset: usize,
}

impl BufferedFile {
    pub fn new() -> Self {
        Self {
            file: RawFile::new(),
            buffer: Vec::new(),
            buffer_available: 0,
            buffer_offset: 0,
        }
    }

    pub fn with_name<P: AsRef<std::path::Path>>(filename: P) -> Self {
        Self {
            file: RawFile::with_name(filename),
            buffer: Vec::new(),
            buffer_available: 0,
            buffer_offset: 0,
        }
    }

    pub fn set_name<P: AsRef<Path>>(&mut self, filename: P) -> &str {
        self.file.set_name(filename)
    }

    pub fn file_name(&self) -> Option<&str> {
        self.file.filename()
    }

    pub fn is_open(&self) -> bool {
        self.file.is_open()
    }

    pub fn is_available(&self, forced: bool) -> bool {
        self.file.is_available(forced)
    }

    pub fn create(&mut self) -> io::Result<bool> {
        self.file.create()
    }

    pub fn delete(&mut self) -> io::Result<bool> {
        self.file.delete()
    }

    pub fn open(&mut self, rights: FileRights) -> io::Result<()> {
        self.file.open(rights)
    }

    pub fn close(&mut self) {
        let _ = self.file.close();
        self.reset_buffer();
    }

    pub fn read(&mut self, out: &mut [u8]) -> usize {
        let mut read = 0usize;
        if self.buffer_available > 0 {
            let amount = out.len().min(self.buffer_available);
            out[..amount]
                .copy_from_slice(&self.buffer[self.buffer_offset..self.buffer_offset + amount]);
            self.buffer_available -= amount;
            self.buffer_offset += amount;
            read += amount;
        }

        if read == out.len() {
            return read;
        }

        if self.buffer.is_empty() {
            self.buffer.resize(16 * 1024, 0u8);
        }

        if out.len() - read > self.buffer.len() {
            if let Ok(count) = self.file.read(&mut out[read..]) {
                return read + count as usize;
            }
            return read;
        }

        if self.buffer_available == 0 {
            match self.file.read(&mut self.buffer) {
                Ok(count) => {
                    self.buffer_available = count as usize;
                    self.buffer_offset = 0;
                }
                Err(_) => return read,
            }
        }

        if self.buffer_available > 0 {
            let amount = (out.len() - read).min(self.buffer_available);
            out[read..read + amount]
                .copy_from_slice(&self.buffer[self.buffer_offset..self.buffer_offset + amount]);
            self.buffer_available -= amount;
            self.buffer_offset += amount;
            read += amount;
        }

        read
    }

    pub fn write(&mut self, data: &[u8]) -> usize {
        if !self.buffer.is_empty() {
            return 0;
        }
        self.file.write(data).unwrap_or(0) as usize
    }

    pub fn bias(&mut self, start: u64, length: Option<u64>) {
        self.file.bias(start, length);
        self.reset_buffer();
    }

    pub fn size(&mut self) -> io::Result<u64> {
        self.file.size()
    }

    pub fn seek(&mut self, pos: i64, origin: SeekOrigin) -> i64 {
        if origin != SeekOrigin::Current || pos < 0 {
            self.reset_buffer();
        }

        if self.buffer_available == 0 {
            return self.file.seek(pos, origin).unwrap_or(0) as i64;
        }

        let amount = (pos as usize).min(self.buffer_available);
        let mut remaining = pos - amount as i64;
        self.buffer_available -= amount;
        self.buffer_offset += amount;

        let seeked = self.file.seek(remaining, origin).unwrap_or(0) as i64;
        seeked - self.buffer_available as i64
    }

    fn reset_buffer(&mut self) {
        self.buffer.clear();
        self.buffer_available = 0;
        self.buffer_offset = 0;
    }
}

impl Default for BufferedFile {
    fn default() -> Self {
        Self::new()
    }
}
