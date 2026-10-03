use crate::{DecodeError, DecodeResult};

pub struct BinaryReader<'a> {
    data: &'a [u8],
    pos: usize,
}

impl<'a> BinaryReader<'a> {
    pub fn position(&self) -> usize {
        self.pos
    }

    pub fn new(data: &'a [u8]) -> Self {
        Self { data, pos: 0 }
    }

    pub fn remaining(&self) -> usize {
        self.data.len().saturating_sub(self.pos)
    }

    pub fn read_bytes(&mut self, len: usize) -> DecodeResult<&'a [u8]> {
        if self.remaining() < len {
            return Err(DecodeError::new("Unexpected end of chunk data"));
        }
        let slice = &self.data[self.pos..self.pos + len];
        self.pos += len;
        Ok(slice)
    }

    pub fn read_u32(&mut self) -> DecodeResult<u32> {
        let bytes = self.read_bytes(4)?;
        Ok(u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
    }

    pub fn read_i32(&mut self) -> DecodeResult<i32> {
        Ok(self.read_u32()? as i32)
    }

    pub fn read_u16(&mut self) -> DecodeResult<u16> {
        let bytes = self.read_bytes(2)?;
        Ok(u16::from_le_bytes([bytes[0], bytes[1]]))
    }

    pub fn read_i16_vec(&mut self, count: usize) -> DecodeResult<Vec<i16>> {
        let bytes = self.read_bytes(count.saturating_mul(2))?;
        Ok(bytes
            .chunks_exact(2)
            .map(|chunk| i16::from_le_bytes([chunk[0], chunk[1]]))
            .collect())
    }

    pub fn read_u8(&mut self) -> DecodeResult<u8> {
        Ok(self.read_bytes(1)?[0])
    }

    pub fn read_f32(&mut self) -> DecodeResult<f32> {
        let bytes = self.read_bytes(4)?;
        Ok(f32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
    }

    pub fn read_ascii_string(&mut self) -> DecodeResult<String> {
        let len = self.read_u16()? as usize;
        let bytes = self.read_bytes(len)?;
        let text = String::from_utf8_lossy(bytes).to_string();
        Ok(text)
    }

    pub fn read_unicode_string(&mut self) -> DecodeResult<String> {
        let len = self.read_u16()? as usize;
        let bytes = self.read_bytes(len.saturating_mul(2))?;
        let mut utf16 = Vec::with_capacity(len);
        for chunk in bytes.chunks_exact(2) {
            utf16.push(u16::from_le_bytes([chunk[0], chunk[1]]));
        }
        Ok(String::from_utf16_lossy(&utf16))
    }

    pub fn take_remaining(&mut self) -> &'a [u8] {
        let slice = &self.data[self.pos..];
        self.pos = self.data.len();
        slice
    }
}
