//! # Texture System Module
//!
//! This module provides texture functionality for the 3D engine,
//! converted from the original texture.cpp/h files.

pub mod asset_texture_loader;
pub mod dds_loader;
pub mod mipmap_generator;
pub mod texfcach;
pub mod texture;
pub mod texture_base;
pub mod texture_filter;
pub mod texture_loader;
pub mod texture_manager;
pub mod texture_sampling;
pub mod texturethumbnail;
pub mod tga_loader;

// Re-export texture types (avoid ambiguity by being specific)
pub use texture::TextureClass;
pub use texture_base::{
    CubeTextureClass, PoolType, TexAssetType, TextureBaseClass, VolumeTextureClass,
};
pub use texture_filter::*;
pub use texture_loader::*;

// Re-export new texture functionality
pub use asset_texture_loader::{AssetTextureCacheStats, AssetTextureLoader};
pub use dds_loader::{load_dds_file, load_dds_from_memory, DdsData, DdsTextureType};
pub use mipmap_generator::{MipmapConfig, MipmapFilter, MipmapGenerator, MipmapLevel};
pub use texfcach::{TextureCacheConfig, TextureCacheStats, TextureFileCache};
pub use texture_manager::{
    TextureManager, TextureManagerHealth, TextureManagerStats, TextureQualitySettings,
};
pub use texture_sampling::{
    TextureAddressMode, TextureFilterQuality, TextureFilteringUtils, TextureSamplerManager,
    TextureSamplingConfig, TextureUsage,
};
pub use tga_loader::{load_tga_file, load_tga_from_memory, TgaData};
