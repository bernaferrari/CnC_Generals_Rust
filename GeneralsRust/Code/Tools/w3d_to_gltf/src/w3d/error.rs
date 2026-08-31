#![allow(dead_code)]
//! Error types for W3D parsing and processing

use std::io;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum W3dError {
    #[error("IO error: {0}")]
    Io(#[from] io::Error),

    #[error("Invalid W3D format: {0}")]
    InvalidFormat(String),

    #[error("Unsupported W3D version: {major}.{minor}")]
    UnsupportedVersion { major: u16, minor: u16 },

    #[error("Invalid chunk type: 0x{chunk_type:08X}")]
    InvalidChunkType { chunk_type: u32 },

    #[error("Chunk size mismatch: expected {expected}, got {actual}")]
    ChunkSizeMismatch { expected: usize, actual: usize },

    #[error("Missing required chunk: {chunk_name}")]
    MissingChunk { chunk_name: String },

    #[error("Invalid data size: expected at least {min_size} bytes, got {actual_size}")]
    InvalidDataSize { min_size: usize, actual_size: usize },

    #[error("Invalid string data: {reason}")]
    InvalidString { reason: String },

    #[error("Invalid mesh data: {reason}")]
    InvalidMeshData { reason: String },

    #[error("Invalid hierarchy data: {reason}")]
    InvalidHierarchyData { reason: String },

    #[error("Invalid animation data: {reason}")]
    InvalidAnimationData { reason: String },

    #[error("Invalid material data: {reason}")]
    InvalidMaterialData { reason: String },

    #[error("Parsing error at offset {offset}: {message}")]
    ParseError { offset: u64, message: String },

    #[error("Conversion error: {message}")]
    ConversionError { message: String },
}

pub type W3dResult<T> = Result<T, W3dError>;

impl W3dError {
    pub fn invalid_format(msg: impl Into<String>) -> Self {
        W3dError::InvalidFormat(msg.into())
    }

    pub fn invalid_mesh_data(msg: impl Into<String>) -> Self {
        W3dError::InvalidMeshData { reason: msg.into() }
    }

    pub fn invalid_hierarchy_data(msg: impl Into<String>) -> Self {
        W3dError::InvalidHierarchyData { reason: msg.into() }
    }

    pub fn invalid_animation_data(msg: impl Into<String>) -> Self {
        W3dError::InvalidAnimationData { reason: msg.into() }
    }

    pub fn invalid_material_data(msg: impl Into<String>) -> Self {
        W3dError::InvalidMaterialData { reason: msg.into() }
    }

    pub fn parse_error(offset: u64, msg: impl Into<String>) -> Self {
        W3dError::ParseError {
            offset,
            message: msg.into(),
        }
    }

    pub fn conversion_error(msg: impl Into<String>) -> Self {
        W3dError::ConversionError {
            message: msg.into(),
        }
    }
}
