//! Renderer-independent SAGE chunky-map byte decoding.
//! C++: Common/System/DataChunk.cpp (TOC, scalar/string reads, parent chunks).
#![forbid(unsafe_code)]

mod chunks;
mod reader;
pub use chunks::{
    CHUNK_HEADER_SIZE, CHUNK_MAGIC, ChunkHeader, find_chunk_by_label, parse_chunk_sequence,
    parse_chunk_toc, read_chunk_header,
};
pub use reader::BinaryReader;
use std::{collections::HashMap, path::PathBuf};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DecodeError(String);
impl DecodeError {
    pub fn new(message: impl Into<String>) -> Self {
        Self(message.into())
    }
}
impl std::fmt::Display for DecodeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}
impl std::error::Error for DecodeError {}
pub type DecodeResult<T> = Result<T, DecodeError>;

/// A load operation owns its decoded bytes. No filesystem or process cache is consulted.
#[derive(Clone)]
pub struct ChunkyMap {
    pub source: PathBuf,
    pub toc: HashMap<u32, String>,
    pub body_offset: usize,
    pub bytes: Vec<u8>,
}
impl ChunkyMap {
    pub fn decode(source: PathBuf, bytes: Vec<u8>) -> DecodeResult<Self> {
        let (toc, body_offset) = parse_chunk_toc(&bytes)?;
        Ok(Self {
            source,
            toc,
            body_offset,
            bytes,
        })
    }
}

#[cfg(test)]
mod tests;
