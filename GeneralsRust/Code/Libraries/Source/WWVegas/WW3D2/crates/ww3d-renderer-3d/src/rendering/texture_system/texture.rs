//! Texture System - Regular 2D Textures
//!
//! This module provides regular 2D texture functionality,
//! equivalent to the original TextureClass.

use crate::core::error::RendererResult;
use crate::core::ww3dformat::FormatDecision;
use crate::core::WW3DFormat;
use crate::rendering::texture_decode::TextureData;
use crate::rendering::texture_system::texture_base::{
    PoolType, TexAssetType, TextureBaseClass, TextureUsagePolicy,
};
use std::sync::Arc;
use wgpu::{Device, Queue, Texture as WgpuTexture};

/// Regular 2D texture class
#[derive(Clone, Debug)]
pub struct TextureClass {
    base: TextureBaseClass,
}

impl TextureClass {
    /// Create new 2D texture
    pub fn new(
        width: u32,
        height: u32,
        format: WW3DFormat,
        mip_level_count: u32,
        pool: PoolType,
    ) -> Self {
        let mut base =
            TextureBaseClass::new(width, height, mip_level_count, pool, TexAssetType::Regular);
        base.set_format(format);
        base.set_name("UnnamedTexture");
        base.set_usage_policy(TextureUsagePolicy::default());

        Self { base }
    }

    /// Populate the texture from decoded CPU data.
    pub fn apply_texture_data(&mut self, data: &TextureData) {
        self.base.apply_texture_data(data);
    }

    /// Create texture from file
    pub fn new_from_file(
        filename: &str,
        mip_level_count: u32,
        texture_format: WW3DFormat,
    ) -> RendererResult<Self> {
        let mut texture = Self::new(1, 1, texture_format, mip_level_count, PoolType::Managed);
        texture.base.set_name(filename);
        texture.base.set_full_path(filename);

        // In a full implementation, this would load the texture from file
        // For now, we just set up the structure
        Ok(texture)
    }

    /// Create texture from raw data
    pub fn new_from_data(
        width: u32,
        height: u32,
        format: WW3DFormat,
        data: &[u8],
        mip_level_count: u32,
    ) -> RendererResult<Self> {
        let mut texture = Self::new(width, height, format, mip_level_count, PoolType::Managed);
        texture
            .base
            .copy_from_slice(width, height, mip_level_count, format, data);
        Ok(texture)
    }

    /// Get texture format
    pub fn format(&self) -> WW3DFormat {
        self.base.ww3d_format
    }

    pub fn format_history(&self) -> Option<&FormatDecision> {
        self.base.format_history()
    }

    /// Set texture format
    pub fn set_format(&mut self, format: WW3DFormat) {
        self.base.set_format(format);
    }

    /// Apply texture to shader stage
    pub fn apply(&self, stage: usize) {
        self.base.apply(stage);
    }

    pub fn set_usage_policy(&mut self, policy: TextureUsagePolicy) {
        self.base.set_usage_policy(policy);
    }

    pub fn usage_policy(&self) -> TextureUsagePolicy {
        self.base.usage_policy()
    }

    /// Ensure GPU resources exist for this texture.
    pub fn ensure_gpu_texture(&mut self, device: &Device, queue: &Queue) -> RendererResult<()> {
        self.base.ensure_gpu_texture(device, queue)
    }

    /// Get underlying WGPU texture
    pub fn wgpu_texture(&self) -> Option<&Arc<WgpuTexture>> {
        self.base.peek_texture()
    }

    /// Check if texture is loaded
    pub fn is_loaded(&self) -> bool {
        self.base.is_loaded()
    }

    /// Get texture memory usage
    pub fn memory_usage(&self) -> usize {
        self.base.get_memory_usage() as usize
    }
}

impl std::ops::Deref for TextureClass {
    type Target = TextureBaseClass;

    fn deref(&self) -> &Self::Target {
        &self.base
    }
}

impl std::ops::DerefMut for TextureClass {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.base
    }
}
