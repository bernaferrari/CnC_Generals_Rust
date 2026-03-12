use crate::error::Result;
use std::io::{Read, Seek, SeekFrom, Write};

/// Chunk identifiers matching WWAudio's Static/Dynamic saves.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum AudioChunkId {
    Static = 0x57415330,  // 'WAS0'
    Dynamic = 0x57414430, // 'WAD0'
}

/// Serializer for audio save data (simple binary format for now).
#[derive(Debug)]
pub struct AudioSaveSerializer<W: Write + Seek> {
    writer: W,
}

impl<W: Write + Seek> AudioSaveSerializer<W> {
    pub fn new(mut writer: W, chunk_id: AudioChunkId) -> Result<Self> {
        writer.write_all(&(chunk_id as u32).to_le_bytes())?;
        writer.write_all(&0u32.to_le_bytes())?; // Placeholder for size
        Ok(Self { writer })
    }

    pub fn write_u32(&mut self, value: u32) -> Result<()> {
        self.writer.write_all(&value.to_le_bytes())?;
        Ok(())
    }

    pub fn write_f32(&mut self, value: f32) -> Result<()> {
        self.writer.write_all(&value.to_le_bytes())?;
        Ok(())
    }

    pub fn write_i32(&mut self, value: i32) -> Result<()> {
        self.writer.write_all(&value.to_le_bytes())?;
        Ok(())
    }

    pub fn write_u64(&mut self, value: u64) -> Result<()> {
        self.writer.write_all(&value.to_le_bytes())?;
        Ok(())
    }

    pub fn write_bytes(&mut self, bytes: &[u8]) -> Result<()> {
        self.writer.write_all(bytes)?;
        Ok(())
    }

    pub fn write_string(&mut self, value: &str) -> Result<()> {
        let bytes = value.as_bytes();
        self.write_u32(bytes.len() as u32)?;
        self.write_bytes(bytes)
    }

    pub fn finish(mut self) -> Result<W> {
        let end_pos = self.writer.seek(SeekFrom::Current(0))?;
        self.writer.seek(SeekFrom::Start(4))?;
        self.writer
            .write_all(&((end_pos as u32) - 8).to_le_bytes())?;
        self.writer.seek(SeekFrom::Start(end_pos))?;
        Ok(self.writer)
    }
}

pub struct AudioLoadDeserializer<R: Read + Seek> {
    reader: R,
    remaining: u32,
}

impl<R: Read + Seek> AudioLoadDeserializer<R> {
    pub fn new(mut reader: R, expected: AudioChunkId) -> Result<Self> {
        let mut id = [0u8; 4];
        reader.read_exact(&mut id)?;
        if u32::from_le_bytes(id) != expected as u32 {
            return Err(crate::AudioError::Audio("Invalid chunk id".to_string()));
        }
        let mut size = [0u8; 4];
        reader.read_exact(&mut size)?;
        let size = u32::from_le_bytes(size);
        Ok(Self {
            reader,
            remaining: size,
        })
    }

    pub fn read_u32(&mut self) -> Result<u32> {
        self.consume(4)?;
        let mut buf = [0u8; 4];
        self.reader.read_exact(&mut buf)?;
        Ok(u32::from_le_bytes(buf))
    }

    pub fn read_f32(&mut self) -> Result<f32> {
        self.consume(4)?;
        let mut buf = [0u8; 4];
        self.reader.read_exact(&mut buf)?;
        Ok(f32::from_le_bytes(buf))
    }

    pub fn read_i32(&mut self) -> Result<i32> {
        self.consume(4)?;
        let mut buf = [0u8; 4];
        self.reader.read_exact(&mut buf)?;
        Ok(i32::from_le_bytes(buf))
    }

    pub fn read_u64(&mut self) -> Result<u64> {
        self.consume(8)?;
        let mut buf = [0u8; 8];
        self.reader.read_exact(&mut buf)?;
        Ok(u64::from_le_bytes(buf))
    }

    pub fn read_string(&mut self) -> Result<String> {
        let len = self.read_u32()? as usize;
        self.consume(len as u32)?;
        let mut buf = vec![0u8; len];
        self.reader.read_exact(&mut buf)?;
        Ok(String::from_utf8(buf).unwrap_or_default())
    }

    pub fn read_remaining_bytes(&mut self) -> Result<Vec<u8>> {
        let len = self.remaining as usize;
        let mut buf = vec![0u8; len];
        self.reader.read_exact(&mut buf)?;
        self.remaining = 0;
        Ok(buf)
    }

    pub fn into_inner(self) -> R {
        self.reader
    }

    fn consume(&mut self, amount: u32) -> Result<()> {
        if self.remaining < amount {
            return Err(crate::AudioError::Audio(
                "Dynamic audio chunk truncated".to_string(),
            ));
        }
        self.remaining -= amount;
        Ok(())
    }
}
