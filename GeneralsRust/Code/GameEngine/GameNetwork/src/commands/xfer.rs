//! Xfer module for data transfer operations
//!
//! This module handles data transfer operations for network commands,
//! including serialization and deserialization of complex data structures.

use crate::error::NetworkResult;
use serde::{Deserialize, Serialize};
use std::io::{Read, Write};

/// Xfer version for compatibility
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum XferVersion {
    /// Version 1
    V1,
    /// Version 2
    V2,
}

impl XferVersion {
    /// Get the current version
    pub const fn current() -> Self {
        Self::V2
    }

    /// Convert to u8
    pub const fn as_u8(&self) -> u8 {
        match self {
            Self::V1 => 1,
            Self::V2 => 2,
        }
    }
}

/// Xfer mode for data transfer
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[repr(u8)]
pub enum XferMode {
    /// Load data
    Load = 0,
    /// Save data
    Save = 1,
}

/// Xfer base trait for serializable objects
pub trait Xfer {
    /// Get the xfer version
    fn xfer_version(&self) -> XferVersion {
        XferVersion::current()
    }

    /// Load data from reader
    fn xfer_load<R: Read>(&mut self, _reader: &mut R, _version: XferVersion) -> NetworkResult<()> {
        Ok(())
    }

    /// Save data to writer
    fn xfer_save<W: Write>(&self, _writer: &mut W) -> NetworkResult<()> {
        Ok(())
    }

    /// Get data size for xfer
    fn xfer_size(&self) -> usize {
        0
    }
}

/// Xfer implementation for basic types
impl Xfer for u32 {
    fn xfer_load<R: Read>(&mut self, reader: &mut R, _version: XferVersion) -> NetworkResult<()> {
        let mut buf = [0u8; 4];
        reader.read_exact(&mut buf)?;
        *self = u32::from_le_bytes(buf);
        Ok(())
    }

    fn xfer_save<W: Write>(&self, writer: &mut W) -> NetworkResult<()> {
        writer.write_all(&self.to_le_bytes())?;
        Ok(())
    }

    fn xfer_size(&self) -> usize {
        4
    }
}

impl Xfer for i32 {
    fn xfer_load<R: Read>(&mut self, reader: &mut R, _version: XferVersion) -> NetworkResult<()> {
        let mut buf = [0u8; 4];
        reader.read_exact(&mut buf)?;
        *self = i32::from_le_bytes(buf);
        Ok(())
    }

    fn xfer_save<W: Write>(&self, writer: &mut W) -> NetworkResult<()> {
        writer.write_all(&self.to_le_bytes())?;
        Ok(())
    }

    fn xfer_size(&self) -> usize {
        4
    }
}

impl Xfer for f32 {
    fn xfer_load<R: Read>(&mut self, reader: &mut R, _version: XferVersion) -> NetworkResult<()> {
        let mut buf = [0u8; 4];
        reader.read_exact(&mut buf)?;
        *self = f32::from_le_bytes(buf);
        Ok(())
    }

    fn xfer_save<W: Write>(&self, writer: &mut W) -> NetworkResult<()> {
        writer.write_all(&self.to_le_bytes())?;
        Ok(())
    }

    fn xfer_size(&self) -> usize {
        4
    }
}

impl Xfer for bool {
    fn xfer_load<R: Read>(&mut self, reader: &mut R, _version: XferVersion) -> NetworkResult<()> {
        let mut buf = [0u8; 1];
        reader.read_exact(&mut buf)?;
        *self = buf[0] != 0;
        Ok(())
    }

    fn xfer_save<W: Write>(&self, writer: &mut W) -> NetworkResult<()> {
        let val = if *self { 1u8 } else { 0u8 };
        writer.write_all(&[val])?;
        Ok(())
    }

    fn xfer_size(&self) -> usize {
        1
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    #[test]
    fn test_u32_xfer() {
        let mut data = 0u32;
        let original = 12345u32;

        let mut buffer = Vec::new();
        original.xfer_save(&mut buffer).unwrap();

        let mut reader = Cursor::new(buffer);
        data.xfer_load(&mut reader, XferVersion::current()).unwrap();

        assert_eq!(data, original);
        assert_eq!(original.xfer_size(), 4);
    }

    #[test]
    fn test_f32_xfer() {
        let mut data = 0.0f32;
        let original = 3.14159f32;

        let mut buffer = Vec::new();
        original.xfer_save(&mut buffer).unwrap();

        let mut reader = Cursor::new(buffer);
        data.xfer_load(&mut reader, XferVersion::current()).unwrap();

        assert!((data - original).abs() < 0.0001);
        assert_eq!(original.xfer_size(), 4);
    }

    #[test]
    fn test_bool_xfer() {
        let mut data = false;
        let original = true;

        let mut buffer = Vec::new();
        original.xfer_save(&mut buffer).unwrap();

        let mut reader = Cursor::new(buffer);
        data.xfer_load(&mut reader, XferVersion::current()).unwrap();

        assert_eq!(data, original);
        assert_eq!(original.xfer_size(), 1);
    }
}
