//! Cube Texture System
//!
//! This module provides cube texture functionality,
//! equivalent to the original CubeTextureClass.

use crate::core::error::{RendererResult, W3dError};
use crate::rendering::texture_system::texture_base::{
    MipCountType, PoolType, TexAssetType, TextureBaseClass, WW3DFormat,
};
use std::sync::Arc;
use wgpu::Texture as WgpuTexture;

/// Cube face enumeration
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum CubeFace {
    PositiveX,
    NegativeX,
    PositiveY,
    NegativeY,
    PositiveZ,
    NegativeZ,
}

/// Cube texture class
#[derive(Clone)]
pub struct CubeTextureClass {
    base: TextureBaseClass,
}

impl CubeTextureClass {
    /// Create new cube texture
    pub fn new(
        width: u32,
        height: u32,
        format: WW3DFormat,
        mip_level_count: MipCountType,
        pool: PoolType,
    ) -> Self {
        let mut base =
            TextureBaseClass::new(width, height, mip_level_count, pool, TexAssetType::Cubemap);
        base.set_name("UnnamedCubeTexture");

        Self { base }
    }

    /// Create cube texture from files (6 faces)
    pub fn new_from_files(
        filenames: [&str; 6],
        mip_level_count: MipCountType,
        texture_format: WW3DFormat,
    ) -> RendererResult<Self> {
        let mut texture = Self::new(1, 1, texture_format, mip_level_count, PoolType::Managed);
        texture.base.set_name("CubeTexture");

        // In a full implementation, this would load all 6 faces
        let _ = filenames; // Suppress unused variable warning

        Ok(texture)
    }

    /// Get face surface
    pub fn get_face_surface(&self, face: CubeFace, level: u32) -> Option<()> {
        let face_index = match face {
            CubeFace::PositiveX => 0usize,
            CubeFace::NegativeX => 1,
            CubeFace::PositiveY => 2,
            CubeFace::NegativeY => 3,
            CubeFace::PositiveZ => 4,
            CubeFace::NegativeZ => 5,
        };
        let mip = self.base.system_mip_levels.get(level as usize)?;
        if mip.depth_or_layers == 0 || face_index as u32 >= mip.depth_or_layers {
            return None;
        }
        let slice_stride = if mip.slice_stride == 0 {
            mip.size
        } else {
            mip.slice_stride
        };
        let begin = mip.offset + face_index * slice_stride;
        let end = begin.checked_add(slice_stride)?;
        if end > self.base.system_memory.len() {
            return None;
        }
        Some(())
    }

    /// Apply cube texture to shader stage
    pub fn apply(&self, stage: usize) {
        self.base.apply(stage);
    }

    /// Get underlying WGPU texture
    pub fn wgpu_texture(&self) -> Option<&Arc<WgpuTexture>> {
        self.base.peek_texture()
    }

    /// Check if texture is loaded
    pub fn is_loaded(&self) -> bool {
        self.base.is_loaded()
    }

    /// Get texture memory usage (6 faces)
    pub fn memory_usage(&self) -> usize {
        // Calculate memory usage for all 6 faces
        let format_size = 4; // Default to 4 bytes per pixel
        (self.base.width * self.base.height * 6 * format_size) as usize
    }
}

impl std::ops::Deref for CubeTextureClass {
    type Target = TextureBaseClass;

    fn deref(&self) -> &Self::Target {
        &self.base
    }
}

impl std::ops::DerefMut for CubeTextureClass {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.base
    }
}
