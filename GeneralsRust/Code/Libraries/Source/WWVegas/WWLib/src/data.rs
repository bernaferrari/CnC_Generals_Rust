// Auto-generated C++ compatibility shim for data buffer
use std::io::{self, Read, Write};

#[derive(Debug, Clone)]
pub struct DataBuffer {
    buf: Vec<u8>,
    cursor: usize,
}

impl DataBuffer {
    pub fn new() -> Self {
        Self {
            buf: Vec::new(),
            cursor: 0,
        }
    }

    pub fn from_bytes(bytes: Vec<u8>) -> Self {
        Self {
            buf: bytes,
            cursor: 0,
        }
    }

    pub fn write_bytes(&mut self, bytes: &[u8]) {
        self.buf.extend_from_slice(bytes);
    }

    pub fn read_bytes(&mut self, len: usize) -> Option<Vec<u8>> {
        if self.cursor + len > self.buf.len() {
            return None;
        }
        let out = self.buf[self.cursor..self.cursor + len].to_vec();
        self.cursor += len;
        Some(out)
    }

    pub fn as_slice(&self) -> &[u8] {
        &self.buf
    }

    pub fn len(&self) -> usize {
        self.buf.len()
    }
}

impl Read for DataBuffer {
    fn read(&mut self, out: &mut [u8]) -> io::Result<usize> {
        let remaining = self.buf.len().saturating_sub(self.cursor);
        let count = remaining.min(out.len());
        out[..count].copy_from_slice(&self.buf[self.cursor..self.cursor + count]);
        self.cursor += count;
        Ok(count)
    }
}

impl Write for DataBuffer {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.buf.extend_from_slice(buf);
        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}
