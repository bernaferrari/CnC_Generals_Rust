//! Sound buffer handling mirroring WWAudio SoundBuffer.cpp/.h.

use crate::error::{Error, Result};
use crate::utils::MMSLockClass;
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};

const WAVE_FORMAT_IMA_ADPCM: u32 = 0x0011;

#[derive(Debug, Clone, Default)]
pub struct SoundBufferClass {
    buffer: Vec<u8>,
    length: u32,
    filename: Option<String>,
    duration_ms: u32,
    rate: u32,
    bits: u32,
    channels: u32,
    sound_type: u32,
}

impl SoundBufferClass {
    pub fn new() -> Self {
        Self {
            buffer: Vec::new(),
            length: 0,
            filename: None,
            duration_ms: 0,
            rate: 0,
            bits: 0,
            channels: 0,
            sound_type: WAVE_FORMAT_IMA_ADPCM,
        }
    }

    pub fn get_raw_buffer(&self) -> &[u8] {
        &self.buffer
    }

    pub fn get_raw_length(&self) -> u32 {
        self.length
    }

    pub fn get_filename(&self) -> Option<&str> {
        self.filename.as_deref()
    }

    pub fn set_filename(&mut self, name: &str) {
        self.filename = Some(name.to_string());
    }

    pub fn get_duration(&self) -> u32 {
        self.duration_ms
    }

    pub fn get_rate(&self) -> u32 {
        self.rate
    }

    pub fn get_bits(&self) -> u32 {
        self.bits
    }

    pub fn get_channels(&self) -> u32 {
        self.channels
    }

    pub fn get_type(&self) -> u32 {
        self.sound_type
    }

    pub fn is_streaming(&self) -> bool {
        false
    }

    pub fn load_from_file(&mut self, filename: &str) -> Result<bool> {
        let mut file = File::open(filename)?;
        self.load_from_reader(&mut file)?;
        self.set_filename(filename);
        Ok(true)
    }

    pub fn load_from_reader<R: Read + Seek>(&mut self, reader: &mut R) -> Result<bool> {
        let _lock = MMSLockClass::new();
        self.free_buffer();

        reader.seek(SeekFrom::Start(0))?;
        let mut data = Vec::new();
        reader.read_to_end(&mut data)?;
        self.length = data.len() as u32;

        if self.length == 0 {
            return Ok(false);
        }

        self.determine_stats(&data)?;
        self.buffer = data;
        Ok(true)
    }

    pub fn load_from_memory(&mut self, mem_buffer: &[u8]) -> Result<bool> {
        let _lock = MMSLockClass::new();
        self.free_buffer();
        self.set_filename("unknown.wav");

        if mem_buffer.is_empty() {
            return Ok(false);
        }

        self.length = mem_buffer.len() as u32;
        self.determine_stats(mem_buffer)?;
        self.buffer = mem_buffer.to_vec();
        Ok(true)
    }

    fn free_buffer(&mut self) {
        self.buffer.clear();
        self.length = 0;
    }

    fn determine_stats(&mut self, buffer: &[u8]) -> Result<()> {
        let _lock = MMSLockClass::new();

        self.duration_ms = 0;
        self.rate = 0;
        self.channels = 0;
        self.bits = 0;
        self.sound_type = WAVE_FORMAT_IMA_ADPCM;

        if buffer.is_empty() {
            return Ok(());
        }

        if let Some((rate, channels, bits, format_tag, bytes_per_sec)) = parse_wav_header(buffer) {
            self.rate = rate;
            self.channels = channels;
            self.bits = bits;
            self.sound_type = format_tag;

            let bytes_per_sec = bytes_per_sec.max(1);
            self.duration_ms = (((self.length as f32) / (bytes_per_sec as f32)) * 1000.0) as u32;
        }

        Ok(())
    }
}

fn parse_wav_header(buffer: &[u8]) -> Option<(u32, u32, u32, u32, u32)> {
    if buffer.len() < 44 {
        return None;
    }
    if &buffer[0..4] != b"RIFF" || &buffer[8..12] != b"WAVE" {
        return None;
    }

    let mut offset = 12;
    let mut format_tag = None;
    let mut channels = None;
    let mut rate = None;
    let mut bits = None;
    let mut bytes_per_sec = None;

    while offset + 8 <= buffer.len() {
        let chunk_type = &buffer[offset..offset + 4];
        let chunk_len = u32::from_le_bytes([
            buffer[offset + 4],
            buffer[offset + 5],
            buffer[offset + 6],
            buffer[offset + 7],
        ]) as usize;
        offset += 8;

        if offset + chunk_len > buffer.len() {
            break;
        }

        match chunk_type {
            b"fmt " if chunk_len >= 16 => {
                format_tag = Some(u16::from_le_bytes([buffer[offset], buffer[offset + 1]]) as u32);
                channels =
                    Some(u16::from_le_bytes([buffer[offset + 2], buffer[offset + 3]]) as u32);
                rate = Some(u32::from_le_bytes([
                    buffer[offset + 4],
                    buffer[offset + 5],
                    buffer[offset + 6],
                    buffer[offset + 7],
                ]));
                bytes_per_sec = Some(u32::from_le_bytes([
                    buffer[offset + 8],
                    buffer[offset + 9],
                    buffer[offset + 10],
                    buffer[offset + 11],
                ]));
                bits = Some(u16::from_le_bytes([buffer[offset + 14], buffer[offset + 15]]) as u32);
            }
            b"data" => break,
            _ => {}
        }

        offset += chunk_len;
        if chunk_len & 1 == 1 {
            offset += 1;
        }
    }

    Some((
        rate?,
        channels?,
        bits?,
        format_tag.unwrap_or(WAVE_FORMAT_IMA_ADPCM),
        bytes_per_sec.unwrap_or(0),
    ))
}

#[derive(Debug, Clone, Default)]
pub struct StreamSoundBufferClass {
    base: SoundBufferClass,
}

impl StreamSoundBufferClass {
    pub fn new() -> Self {
        Self {
            base: SoundBufferClass::new(),
        }
    }

    pub fn load_from_file(&mut self, filename: &str) -> Result<bool> {
        let mut file = File::open(filename)?;
        self.load_from_reader(&mut file)?;
        self.base.set_filename(filename);
        Ok(true)
    }

    pub fn load_from_reader<R: Read + Seek>(&mut self, reader: &mut R) -> Result<bool> {
        let _lock = MMSLockClass::new();
        self.base.free_buffer();

        reader.seek(SeekFrom::Start(0))?;
        let length = reader.seek(SeekFrom::End(0))?;
        self.base.length = length as u32;
        reader.seek(SeekFrom::Start(0))?;

        let mut preview = vec![0u8; 4096];
        let bytes_read = reader.read(&mut preview)?;
        preview.truncate(bytes_read);
        self.base.determine_stats(&preview)?;
        Ok(true)
    }

    pub fn load_from_memory(&mut self, _mem_buffer: &[u8]) -> Result<bool> {
        Ok(false)
    }

    pub fn is_streaming(&self) -> bool {
        true
    }

    pub fn base(&self) -> &SoundBufferClass {
        &self.base
    }

    pub fn base_mut(&mut self) -> &mut SoundBufferClass {
        &mut self.base
    }
}
