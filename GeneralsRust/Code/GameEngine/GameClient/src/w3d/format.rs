//! W3D File Format Support

use super::{W3DError, W3DResult};
use std::io::{Cursor, Read};

/// W3D File Format Loader
pub struct W3DLoader {
    // Implementation for loading original W3D files
}

/// W3D File Format Types
pub enum W3DFileFormat {
    Mesh,      // .w3d mesh files
    Hierarchy, // .w3d hierarchy files
    Animation, // .w3d animation files
}

/// W3D Chunk Types (from original format specification)
pub struct W3DChunk {
    pub chunk_type: u32,
    pub chunk_size: u32,
    pub data: Vec<u8>,
}

impl W3DLoader {
    pub fn new() -> Self {
        Self {}
    }

    pub async fn load_w3d_file(&self, path: &str) -> W3DResult<Vec<W3DChunk>> {
        let bytes = std::fs::read(path)
            .map_err(|err| W3DError::InvalidFormat(format!("Failed to read '{path}': {err}")))?;

        if bytes.len() < 8 {
            return Err(W3DError::InvalidFormat(format!(
                "W3D file '{path}' is too small to contain chunk headers"
            )));
        }

        let mut cursor = Cursor::new(bytes.as_slice());
        let mut chunks = Vec::new();

        while (cursor.position() as usize) + 8 <= bytes.len() {
            let mut header = [0u8; 8];
            cursor.read_exact(&mut header).map_err(|err| {
                W3DError::InvalidFormat(format!("Failed to read chunk header in '{path}': {err}"))
            })?;

            let chunk_type = u32::from_le_bytes([header[0], header[1], header[2], header[3]]);
            let chunk_size = u32::from_le_bytes([header[4], header[5], header[6], header[7]]);
            let chunk_size_usize = chunk_size as usize;
            let remaining = bytes.len().saturating_sub(cursor.position() as usize);

            if chunk_size_usize > remaining {
                return Err(W3DError::InvalidFormat(format!(
                    "Chunk 0x{chunk_type:08X} in '{path}' declares {chunk_size_usize} bytes, but only {remaining} remain"
                )));
            }

            let mut data = vec![0u8; chunk_size_usize];
            cursor.read_exact(&mut data).map_err(|err| {
                W3DError::InvalidFormat(format!(
                    "Failed to read chunk payload 0x{chunk_type:08X} in '{path}': {err}"
                ))
            })?;

            chunks.push(W3DChunk {
                chunk_type,
                chunk_size,
                data,
            });
        }

        if chunks.is_empty() {
            return Err(W3DError::InvalidFormat(format!(
                "W3D file '{path}' did not contain any readable chunks"
            )));
        }

        Ok(chunks)
    }
}
