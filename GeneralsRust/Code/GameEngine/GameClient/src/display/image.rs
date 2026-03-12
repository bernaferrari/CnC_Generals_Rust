//! # Image Module
//!
//! High-level image representation and texture management system.
//! Converts the original Image class to modern Rust with support for various formats.

use bitflags::bitflags;
use image::{DynamicImage, GenericImageView, ImageBuffer, ImageError, ImageFormat, RgbaImage};
use nalgebra::{Point2, Vector2};
use once_cell::sync::OnceCell;
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::path::{Component, Path, PathBuf};
use std::sync::{Arc, Mutex};
use thiserror::Error;
use wgpu::{Device, Queue, Sampler, Texture, TextureView};

use crate::system::SubsystemInterface;
use game_engine::common::global_data;
use game_engine::common::ini::ini_game_data::get_global_data as get_runtime_global_data;
use game_engine::common::ini::ini_mapped_image::{
    get_mapped_image_collection as get_common_mapped_image_collection,
    ImageCollection as CommonImageCollection,
};
use game_engine::common::system::big_file_system::BigArchiveBackend;
use game_engine::common::system::file::FileAccess;
use game_engine::common::system::file_system::get_file_system;
use game_engine::common::system::local_file_system::LocalFileSystem;
use game_engine::common::system::subsystem_interface::{
    SubsystemInterface as CommonSubsystemInterface, SubsystemState,
};

fn is_startup_shell_image(name: &str) -> bool {
    matches!(
        name,
        "MainMenuBackdrop" | "MainMenuPulse" | "GeneralsLogo" | "MainMenuRuler" | "BlackSquare"
    )
}

fn log_startup_shell_image_once(name: &str, message: String) {
    static REPORTED: OnceCell<Mutex<HashSet<String>>> = OnceCell::new();
    let reported = REPORTED.get_or_init(|| Mutex::new(HashSet::new()));
    let key = format!("{name}:{message}");
    let Ok(mut guard) = reported.lock() else {
        return;
    };
    if guard.insert(key) {
        eprintln!("DEBUG_STARTUP_IMAGE: name={name} {message}");
    }
}

/// Image-related error types
#[derive(Error, Debug)]
pub enum GameImageError {
    #[error("Failed to load image from file '{path}': {source}")]
    LoadError {
        path: String,
        source: Box<dyn std::error::Error + Send + Sync>,
    },
    #[error("Failed to create GPU texture: {0}")]
    TextureCreation(String),
    #[error("Invalid image format: {0}")]
    InvalidFormat(String),
    #[error("Image not found: {0}")]
    ImageNotFound(String),
    #[error("Invalid UV coordinates: {uv:?}")]
    InvalidUV { uv: Region2D },
    #[error("Memory allocation failed for image data")]
    MemoryAllocation,
    #[error("GPU resource creation failed: {0}")]
    GPUResourceError(String),
}

bitflags! {
    /// Image status bits
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct ImageStatus: u32 {
        const NONE = 0x00000000;
        /// Image should be treated as rotated 90 degrees clockwise
        const ROTATED_90_CLOCKWISE = 0x00000001;
        /// Image struct contains raw texture data
        const RAW_TEXTURE = 0x00000002;
        /// Image has been loaded into GPU memory
        const GPU_LOADED = 0x00000004;
        /// Image supports transparency/alpha channel
        const HAS_ALPHA = 0x00000008;
        /// Image has been compressed
        const COMPRESSED = 0x00000010;
    }
}

/// 2D Region for UV coordinates and positioning
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct Region2D {
    pub min: Point2<f32>,
    pub max: Point2<f32>,
}

impl Region2D {
    pub fn new(x: f32, y: f32, width: f32, height: f32) -> Self {
        Self {
            min: Point2::new(x, y),
            max: Point2::new(x + width, y + height),
        }
    }

    pub fn from_coords(left: f32, top: f32, right: f32, bottom: f32) -> Self {
        Self {
            min: Point2::new(left, top),
            max: Point2::new(right, bottom),
        }
    }

    pub fn width(&self) -> f32 {
        self.max.x - self.min.x
    }

    pub fn height(&self) -> f32 {
        self.max.y - self.min.y
    }

    pub fn normalized_from_pixel_coords(
        left: i32,
        top: i32,
        right: i32,
        bottom: i32,
        texture_width: i32,
        texture_height: i32,
    ) -> Self {
        Self::from_coords(
            left as f32 / texture_width as f32,
            top as f32 / texture_height as f32,
            right as f32 / texture_width as f32,
            bottom as f32 / texture_height as f32,
        )
    }
}

impl Default for Region2D {
    fn default() -> Self {
        Self::from_coords(0.0, 0.0, 1.0, 1.0)
    }
}

/// 2D Integer coordinates
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct ICoord2D {
    pub x: i32,
    pub y: i32,
}

impl ICoord2D {
    pub const ZERO: Self = Self { x: 0, y: 0 };

    pub fn new(x: i32, y: i32) -> Self {
        Self { x, y }
    }

    pub fn zero() -> Self {
        Self::ZERO
    }
}

/// GPU texture resource wrapper
#[derive(Debug)]
pub struct GPUTexture {
    texture: Texture,
    view: TextureView,
    sampler: Sampler,
}

impl GPUTexture {
    pub fn new(device: &Device, texture: Texture, sampler: Sampler) -> Self {
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        Self {
            texture,
            view,
            sampler,
        }
    }

    pub fn texture(&self) -> &Texture {
        &self.texture
    }

    pub fn view(&self) -> &TextureView {
        &self.view
    }

    pub fn sampler(&self) -> &Sampler {
        &self.sampler
    }
}

/// High-level image representation
#[derive(Debug)]
pub struct Image {
    /// Image name/identifier
    name: String,
    /// Source filename
    filename: String,
    /// Texture page dimensions this image is part of
    texture_size: ICoord2D,
    /// UV coordinates within the texture
    uv_coords: Region2D,
    /// Image dimensions in pixels
    image_size: ICoord2D,
    /// Status flags
    status: ImageStatus,
    /// Raw image data (CPU-side)
    image_data: Option<DynamicImage>,
    /// GPU texture resource
    gpu_texture: Option<Arc<GPUTexture>>,
}

impl Image {
    /// Create a new empty image
    pub fn new() -> Self {
        Self {
            name: String::new(),
            filename: String::new(),
            texture_size: ICoord2D::ZERO,
            uv_coords: Region2D::default(),
            image_size: ICoord2D::ZERO,
            status: ImageStatus::NONE,
            image_data: None,
            gpu_texture: None,
        }
    }

    /// Create an image with a name
    pub fn with_name(name: impl Into<String>) -> Self {
        let mut image = Self::new();
        image.name = name.into();
        image
    }

    /// Load image from file
    pub fn load_from_file<P: AsRef<Path>>(
        path: P,
        name: Option<String>,
    ) -> Result<Self, GameImageError> {
        let path = path.as_ref();
        let path_str = path.to_string_lossy().to_string();

        let image_data = image::open(path).map_err(|e| GameImageError::LoadError {
            path: path_str.clone(),
            source: Box::new(e),
        })?;

        let (width, height) = image_data.dimensions();
        let has_alpha = image_data.color().has_alpha();

        let mut status = ImageStatus::NONE;
        if has_alpha {
            status |= ImageStatus::HAS_ALPHA;
        }

        let image_name = name.unwrap_or_else(|| {
            path.file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("unnamed")
                .to_string()
        });

        Ok(Self {
            name: image_name,
            filename: path_str,
            texture_size: ICoord2D::new(width as i32, height as i32),
            uv_coords: Region2D::default(),
            image_size: ICoord2D::new(width as i32, height as i32),
            status,
            image_data: Some(image_data),
            gpu_texture: None,
        })
    }

    /// Load image from raw RGBA data
    pub fn from_rgba_data(
        data: &[u8],
        width: u32,
        height: u32,
        name: impl Into<String>,
    ) -> Result<Self, GameImageError> {
        let image_buffer = ImageBuffer::from_raw(width, height, data.to_vec())
            .ok_or(GameImageError::MemoryAllocation)?;

        let dynamic_image = DynamicImage::ImageRgba8(image_buffer);

        Ok(Self {
            name: name.into(),
            filename: String::new(),
            texture_size: ICoord2D::new(width as i32, height as i32),
            uv_coords: Region2D::default(),
            image_size: ICoord2D::new(width as i32, height as i32),
            status: ImageStatus::HAS_ALPHA | ImageStatus::RAW_TEXTURE,
            image_data: Some(dynamic_image),
            gpu_texture: None,
        })
    }

    /// Create GPU texture from image data
    pub fn create_gpu_texture(
        &mut self,
        device: &Device,
        queue: &Queue,
    ) -> Result<(), GameImageError> {
        if self.gpu_texture.is_some() {
            return Ok(());
        }

        if let Err(error) = self.ensure_image_data_loaded() {
            if is_startup_shell_image(&self.name) {
                log_startup_shell_image_once(
                    &self.name,
                    format!("load_failed file={} error={}", self.filename, error),
                );
            }
            return Err(error);
        }
        let image_data = self
            .image_data
            .as_ref()
            .ok_or(GameImageError::TextureCreation(
                "No image data available".to_string(),
            ))?;

        let rgba = image_data.to_rgba8();
        let dimensions = rgba.dimensions();

        let texture_size = wgpu::Extent3d {
            width: dimensions.0,
            height: dimensions.1,
            depth_or_array_layers: 1,
        };

        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some(&format!("Texture_{}", self.name)),
            size: texture_size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });

        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &rgba,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(4 * dimensions.0),
                rows_per_image: Some(dimensions.1),
            },
            texture_size,
        );

        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some(&format!("Sampler_{}", self.name)),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });

        self.gpu_texture = Some(Arc::new(GPUTexture::new(device, texture, sampler)));
        self.status |= ImageStatus::GPU_LOADED;
        if is_startup_shell_image(&self.name) {
            log_startup_shell_image_once(
                &self.name,
                format!(
                    "gpu_loaded file={} size={}x{}",
                    self.filename, dimensions.0, dimensions.1
                ),
            );
        }

        Ok(())
    }

    // Getters and setters
    pub fn set_name(&mut self, name: impl Into<String>) {
        self.name = name.into();
    }

    pub fn get_name(&self) -> &str {
        &self.name
    }

    pub fn set_filename(&mut self, filename: impl Into<String>) {
        self.filename = filename.into();
    }

    pub fn get_filename(&self) -> &str {
        &self.filename
    }

    pub fn set_uv(&mut self, uv: Region2D) {
        self.uv_coords = uv;
    }

    pub fn get_uv(&self) -> &Region2D {
        &self.uv_coords
    }

    pub fn set_texture_width(&mut self, width: i32) {
        self.texture_size.x = width;
    }

    pub fn set_texture_height(&mut self, height: i32) {
        self.texture_size.y = height;
    }

    pub fn get_texture_size(&self) -> &ICoord2D {
        &self.texture_size
    }

    pub fn set_image_size(&mut self, size: ICoord2D) {
        self.image_size = size;
    }

    pub fn get_image_size(&self) -> &ICoord2D {
        &self.image_size
    }

    pub fn get_image_width(&self) -> i32 {
        self.image_size.x
    }

    pub fn get_image_height(&self) -> i32 {
        self.image_size.y
    }

    pub fn set_status(&mut self, status: ImageStatus) -> ImageStatus {
        let previous = self.status;
        self.status |= status;
        previous
    }

    pub fn clear_status(&mut self, status: ImageStatus) -> ImageStatus {
        let previous = self.status;
        self.status &= !status;
        previous
    }

    pub fn get_status(&self) -> ImageStatus {
        self.status
    }

    pub fn has_status(&self, status: ImageStatus) -> bool {
        self.status.contains(status)
    }

    /// Get the GPU texture if available
    pub fn get_gpu_texture(&self) -> Option<&Arc<GPUTexture>> {
        self.gpu_texture.as_ref()
    }

    /// Get the raw image data
    pub fn get_image_data(&self) -> Option<&DynamicImage> {
        self.image_data.as_ref()
    }

    /// Check if image has alpha channel
    pub fn has_alpha(&self) -> bool {
        self.status.contains(ImageStatus::HAS_ALPHA)
    }

    /// Check if image is loaded on GPU
    pub fn is_gpu_loaded(&self) -> bool {
        self.status.contains(ImageStatus::GPU_LOADED)
    }

    /// Set image coordinates and compute UV coordinates
    pub fn set_coords(&mut self, left: i32, top: i32, right: i32, bottom: i32) {
        // Store pixel-based image size
        self.image_size = ICoord2D::new(right - left, bottom - top);

        // Compute normalized UV coordinates if texture size is set
        if self.texture_size.x > 0 && self.texture_size.y > 0 {
            self.uv_coords = Region2D::normalized_from_pixel_coords(
                left,
                top,
                right,
                bottom,
                self.texture_size.x,
                self.texture_size.y,
            );
        }
    }

    /// Convert to grayscale
    pub fn to_grayscale(&mut self) {
        if let Some(ref mut image_data) = self.image_data {
            *image_data = image_data.grayscale();
            // Clear alpha status since grayscale removes alpha
            self.status &= !ImageStatus::HAS_ALPHA;
        }
    }

    /// Resize image
    pub fn resize(&mut self, width: u32, height: u32, filter: image::imageops::FilterType) {
        if let Some(ref mut image_data) = self.image_data {
            *image_data = image_data.resize(width, height, filter);
            self.image_size = ICoord2D::new(width as i32, height as i32);
            // Clear GPU loaded status since we need to re-upload
            self.status &= !ImageStatus::GPU_LOADED;
            self.gpu_texture = None;
        }
    }

    fn ensure_image_data_loaded(&mut self) -> Result<(), GameImageError> {
        if self.image_data.is_some() {
            return Ok(());
        }

        if self.filename.is_empty() {
            return Err(GameImageError::ImageNotFound(self.name.clone()));
        }

        let virtual_candidates = candidate_texture_resource_names(&self.filename);
        for candidate in &virtual_candidates {
            if let Some(decoded) = try_load_image_from_engine_filesystem(candidate) {
                let (width, height) = decoded.dimensions();
                if self.texture_size.x == 0 || self.texture_size.y == 0 {
                    self.texture_size = ICoord2D::new(width as i32, height as i32);
                }
                if self.image_size.x == 0 || self.image_size.y == 0 {
                    self.image_size = ICoord2D::new(width as i32, height as i32);
                }
                if decoded.color().has_alpha() {
                    self.status |= ImageStatus::HAS_ALPHA;
                }
                self.image_data = Some(decoded);
                return Ok(());
            }
        }

        let fallback_virtual_candidates =
            startup_shell_fallback_resource_names(&self.name, &self.filename);
        for candidate in &fallback_virtual_candidates {
            if let Some(decoded) = try_load_image_from_engine_filesystem(candidate) {
                let requested = self.filename.clone();
                let (width, height) = decoded.dimensions();
                if self.texture_size.x == 0 || self.texture_size.y == 0 {
                    self.texture_size = ICoord2D::new(width as i32, height as i32);
                }
                if self.image_size.x == 0 || self.image_size.y == 0 {
                    self.image_size = ICoord2D::new(width as i32, height as i32);
                }
                if decoded.color().has_alpha() {
                    self.status |= ImageStatus::HAS_ALPHA;
                }
                self.image_data = Some(decoded);
                self.filename = candidate.clone();
                if is_startup_shell_image(&self.name) {
                    log_startup_shell_image_once(
                        &self.name,
                        format!("fallback_loaded requested={requested} resolved={candidate}"),
                    );
                }
                return Ok(());
            }
        }

        let candidates = candidate_texture_paths(&self.filename, &virtual_candidates);
        for path in candidates {
            let Some(path) = resolve_case_insensitive_path(&path) else {
                continue;
            };

            let loaded = image::open(&path).map_err(|source| GameImageError::LoadError {
                path: path.to_string_lossy().to_string(),
                source: Box::new(source),
            })?;

            let (width, height) = loaded.dimensions();
            if self.texture_size.x == 0 || self.texture_size.y == 0 {
                self.texture_size = ICoord2D::new(width as i32, height as i32);
            }
            if self.image_size.x == 0 || self.image_size.y == 0 {
                self.image_size = ICoord2D::new(width as i32, height as i32);
            }
            if loaded.color().has_alpha() {
                self.status |= ImageStatus::HAS_ALPHA;
            }
            self.image_data = Some(loaded);
            return Ok(());
        }

        let fallback_paths = candidate_texture_paths(&self.filename, &fallback_virtual_candidates);
        for path in fallback_paths {
            let Some(path) = resolve_case_insensitive_path(&path) else {
                continue;
            };
            let requested = self.filename.clone();

            let loaded = image::open(&path).map_err(|source| GameImageError::LoadError {
                path: path.to_string_lossy().to_string(),
                source: Box::new(source),
            })?;

            let (width, height) = loaded.dimensions();
            if self.texture_size.x == 0 || self.texture_size.y == 0 {
                self.texture_size = ICoord2D::new(width as i32, height as i32);
            }
            if self.image_size.x == 0 || self.image_size.y == 0 {
                self.image_size = ICoord2D::new(width as i32, height as i32);
            }
            if loaded.color().has_alpha() {
                self.status |= ImageStatus::HAS_ALPHA;
            }
            self.image_data = Some(loaded);
            self.filename = path.to_string_lossy().to_string();
            if is_startup_shell_image(&self.name) {
                log_startup_shell_image_once(
                    &self.name,
                    format!(
                        "fallback_loaded requested={} resolved={}",
                        requested,
                        path.to_string_lossy()
                    ),
                );
            }
            return Ok(());
        }

        if self.name.eq_ignore_ascii_case("MainMenuBackdrop") {
            let width = self.texture_size.x.max(1024) as u32;
            let height = self.texture_size.y.max(1024) as u32;
            self.image_data = Some(DynamicImage::ImageRgba8(ImageBuffer::from_pixel(
                width,
                height,
                image::Rgba([0, 0, 0, 0]),
            )));
            self.filename = "__synthetic_transparent_main_menu_backdrop__".to_string();
            self.status |= ImageStatus::HAS_ALPHA;
            if is_startup_shell_image(&self.name) {
                log_startup_shell_image_once(
                    &self.name,
                    "fallback_loaded requested=MainMenuBackdrop synthetic=transparent".to_string(),
                );
            }
            return Ok(());
        }

        if self.name.eq_ignore_ascii_case("BlackSquare") {
            self.image_data = Some(DynamicImage::ImageRgba8(ImageBuffer::from_pixel(
                1,
                1,
                image::Rgba([255, 255, 255, 255]),
            )));
            self.filename = "__synthetic_black_square__".to_string();
            self.texture_size = ICoord2D::new(1, 1);
            self.image_size = ICoord2D::new(1, 1);
            self.status |= ImageStatus::HAS_ALPHA;
            if is_startup_shell_image(&self.name) {
                log_startup_shell_image_once(
                    &self.name,
                    "fallback_loaded requested=BlackSquare synthetic=white".to_string(),
                );
            }
            return Ok(());
        }

        Err(GameImageError::ImageNotFound(self.filename.clone()))
    }
}

impl Default for Image {
    fn default() -> Self {
        Self::new()
    }
}

/// Collection of images for asset management
#[derive(Debug)]
pub struct ImageCollection {
    images: HashMap<String, Image>,
    texture_size: i32,
}

impl ImageCollection {
    /// Create a new image collection
    pub fn new() -> Self {
        Self {
            images: HashMap::new(),
            texture_size: 512, // Default texture atlas size
        }
    }

    /// Create collection with specified texture atlas size
    pub fn with_texture_size(texture_size: i32) -> Self {
        Self {
            images: HashMap::new(),
            texture_size,
        }
    }

    /// Load images from a directory
    pub fn load_from_directory<P: AsRef<Path>>(
        &mut self,
        path: P,
        recursive: bool,
    ) -> Result<usize, GameImageError> {
        let mut loaded_count = 0;
        let path = path.as_ref();

        if !path.is_dir() {
            return Ok(0);
        }

        let entries = if recursive {
            path.read_dir()
                .map_err(|e| GameImageError::LoadError {
                    path: path.to_string_lossy().to_string(),
                    source: Box::new(e),
                })?
                .collect::<Result<Vec<_>, _>>()
                .map_err(|e| GameImageError::LoadError {
                    path: path.to_string_lossy().to_string(),
                    source: Box::new(e),
                })?
        } else {
            vec![]
        };

        for entry in entries {
            let path = entry.path();
            if path.is_file() {
                if let Some(extension) = path.extension() {
                    let ext_str = extension.to_string_lossy().to_lowercase();
                    if matches!(ext_str.as_str(), "png" | "tga" | "jpg" | "jpeg" | "bmp") {
                        match Image::load_from_file(&path, None) {
                            Ok(image) => {
                                let name = image.get_name().to_string();
                                self.add_image(image);
                                loaded_count += 1;
                                log::debug!("Loaded image: {}", name);
                            }
                            Err(e) => {
                                log::warn!("Failed to load image {}: {}", path.display(), e);
                            }
                        }
                    }
                }
            }
        }

        Ok(loaded_count)
    }

    /// Add an image to the collection
    pub fn add_image(&mut self, image: Image) {
        let name = image.get_name().to_lowercase();
        self.images.insert(name, image);
    }

    /// Find an image by name
    pub fn find_image_by_name(&self, name: &str) -> Option<&Image> {
        let key = name.to_lowercase();
        self.images.get(&key)
    }

    /// Find an image by name (mutable)
    pub fn find_image_by_name_mut(&mut self, name: &str) -> Option<&mut Image> {
        let key = name.to_lowercase();
        self.images.get_mut(&key)
    }

    /// Get image by index (for enumeration)
    pub fn get_image_by_index(&self, index: usize) -> Option<&Image> {
        self.images.values().nth(index)
    }

    /// Get number of images in collection
    pub fn count(&self) -> usize {
        self.images.len()
    }

    /// Get all image names
    pub fn get_image_names(&self) -> Vec<&String> {
        self.images.keys().collect()
    }

    /// Remove an image from collection
    pub fn remove_image(&mut self, name: &str) -> Option<Image> {
        let key = name.to_lowercase();
        self.images.remove(&key)
    }

    /// Clear all images
    pub fn clear(&mut self) {
        self.images.clear();
    }

    /// Create GPU textures for all images
    pub fn create_gpu_textures(
        &mut self,
        device: &Device,
        queue: &Queue,
    ) -> Result<usize, GameImageError> {
        let mut success_count = 0;

        for image in self.images.values_mut() {
            if !image.is_gpu_loaded() {
                match image.create_gpu_texture(device, queue) {
                    Ok(()) => success_count += 1,
                    Err(e) => log::warn!(
                        "Failed to create GPU texture for {}: {}",
                        image.get_name(),
                        e
                    ),
                }
            }
        }

        Ok(success_count)
    }

    /// Get texture size setting
    pub fn get_texture_size(&self) -> i32 {
        self.texture_size
    }

    /// Set texture size setting
    pub fn set_texture_size(&mut self, size: i32) {
        self.texture_size = size;
    }

    /// Preload specific images
    pub fn preload_images(
        &mut self,
        names: &[&str],
        device: &Device,
        queue: &Queue,
    ) -> Result<usize, GameImageError> {
        let mut loaded_count = 0;

        for name in names {
            if let Some(image) = self.find_image_by_name_mut(name) {
                if !image.is_gpu_loaded() {
                    image.create_gpu_texture(device, queue)?;
                    loaded_count += 1;
                }
            }
        }

        Ok(loaded_count)
    }

    /// Get memory usage statistics
    pub fn get_memory_stats(&self) -> (usize, usize) {
        let mut cpu_memory = 0;
        let mut gpu_textures = 0;

        for image in self.images.values() {
            if let Some(image_data) = image.get_image_data() {
                let (width, height) = image_data.dimensions();
                let channels = match image_data.color() {
                    image::ColorType::Rgb8 => 3,
                    image::ColorType::Rgba8 => 4,
                    image::ColorType::L8 => 1,
                    image::ColorType::La8 => 2,
                    _ => 4, // Default to 4 for other formats
                };
                cpu_memory += (width * height * channels) as usize;
            }

            if image.is_gpu_loaded() {
                gpu_textures += 1;
            }
        }

        (cpu_memory, gpu_textures)
    }
}

impl Default for ImageCollection {
    fn default() -> Self {
        Self::new()
    }
}

impl SubsystemInterface for ImageCollection {
    fn init(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        log::info!("Initializing ImageCollection subsystem");
        Ok(())
    }

    fn reset(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        log::info!("Resetting ImageCollection subsystem");
        self.clear();
        Ok(())
    }

    fn update(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        // Perform any per-frame image collection updates
        Ok(())
    }
}

/// Global image collection instance (thread-safe)
static MAPPED_IMAGE_COLLECTION: OnceCell<Arc<RwLock<ImageCollection>>> = OnceCell::new();

/// Ensure the mapped image collection exists and return a handle to it
pub fn ensure_mapped_image_collection() -> Arc<RwLock<ImageCollection>> {
    MAPPED_IMAGE_COLLECTION
        .get_or_init(|| Arc::new(RwLock::new(ImageCollection::new())))
        .clone()
}

fn candidate_texture_resource_names(filename: &str) -> Vec<String> {
    let normalized = filename.replace('\\', "/");
    let bare = normalized.trim_start_matches("./").to_string();
    let has_extension = Path::new(&bare).extension().is_some();
    let mut candidates = Vec::new();
    let mut push_unique = |list: &mut Vec<String>, candidate: String| {
        if !list.iter().any(|existing| existing == &candidate) {
            list.push(candidate);
        }
    };

    if !bare.is_empty() {
        push_unique(&mut candidates, bare.clone());
    }

    // C++ parity: mapped image texture names are often bare filenames that resolve
    // under Art/Textures via search paths/backends.
    if !bare.contains('/') {
        push_unique(&mut candidates, format!("Art/Textures/{bare}"));
        push_unique(&mut candidates, format!("Art/Terrain/{bare}"));
        push_unique(&mut candidates, format!("English/Art/Textures/{bare}"));
        push_unique(&mut candidates, format!("Data/Art/Textures/{bare}"));
        push_unique(&mut candidates, format!("Data/Art/Terrain/{bare}"));
        push_unique(&mut candidates, format!("Data/English/Art/Textures/{bare}"));
    }

    if !bare.starts_with("Data/") {
        push_unique(&mut candidates, format!("Data/{bare}"));
    }

    if !has_extension {
        let base_candidates = candidates.clone();
        for base in &base_candidates {
            for ext in ["tga", "dds", "png", "jpg", "jpeg", "bmp"] {
                push_unique(&mut candidates, format!("{base}.{ext}"));
            }
        }
    }

    candidates
}

fn candidate_texture_paths(filename: &str, virtual_candidates: &[String]) -> Vec<PathBuf> {
    let mut paths = Vec::new();
    let mut push_unique = |candidate: PathBuf| {
        if !paths.iter().any(|existing| existing == &candidate) {
            paths.push(candidate);
        }
    };

    for candidate in virtual_candidates {
        push_unique(PathBuf::from(candidate));
    }

    if !filename.is_empty() {
        push_unique(PathBuf::from(filename));
    }

    for root in runtime_texture_search_roots() {
        for candidate in virtual_candidates {
            push_unique(root.join(candidate));
        }
    }

    paths
}

fn resolve_case_insensitive_path(path: &Path) -> Option<PathBuf> {
    if path.exists() {
        return Some(path.to_path_buf());
    }

    let mut resolved = PathBuf::new();

    for component in path.components() {
        match component {
            Component::Prefix(prefix) => resolved.push(prefix.as_os_str()),
            Component::RootDir => resolved.push(component.as_os_str()),
            Component::CurDir => {}
            Component::ParentDir => {
                if !resolved.pop() {
                    return None;
                }
            }
            Component::Normal(part) => {
                let exact = resolved.join(part);
                if exact.exists() {
                    resolved = exact;
                    continue;
                }

                let search_dir = if resolved.as_os_str().is_empty() {
                    Path::new(".")
                } else {
                    resolved.as_path()
                };
                let part = part.to_string_lossy();
                let entries = search_dir.read_dir().ok()?;
                let matched = entries.filter_map(Result::ok).find_map(|entry| {
                    let name = entry.file_name();
                    if name.to_string_lossy().eq_ignore_ascii_case(&part) {
                        Some(entry.path())
                    } else {
                        None
                    }
                })?;
                resolved = matched;
            }
        }
    }

    resolved.exists().then_some(resolved)
}

fn runtime_root_candidates() -> Vec<PathBuf> {
    let mut roots = Vec::new();
    if let Ok(cwd) = std::env::current_dir() {
        roots.push(cwd);
    }
    if let Ok(exe) = std::env::current_exe() {
        if let Some(parent) = exe.parent() {
            roots.push(parent.to_path_buf());
        }
    }
    roots
}

fn candidate_texture_search_roots_from_base(base: &Path) -> Vec<PathBuf> {
    let mut roots = vec![
        PathBuf::from("windows_game/extracted_big_files/EnglishZH"),
        PathBuf::from("windows_game/extracted_big_files/TexturesZH"),
        PathBuf::from("windows_game/extracted_big_files_v2/EnglishZH"),
        PathBuf::from("windows_game/extracted_big_files_v2/TexturesZH"),
        PathBuf::from("windows_game/Command & Conquer Generals Zero Hour"),
    ];

    for ancestor in base.ancestors() {
        roots.push(ancestor.join("windows_game/extracted_big_files/EnglishZH"));
        roots.push(ancestor.join("windows_game/extracted_big_files/TexturesZH"));
        roots.push(ancestor.join("windows_game/extracted_big_files_v2/EnglishZH"));
        roots.push(ancestor.join("windows_game/extracted_big_files_v2/TexturesZH"));
        roots.push(ancestor.join("windows_game/Command & Conquer Generals Zero Hour"));
    }

    roots
}

fn runtime_texture_search_roots() -> Vec<PathBuf> {
    let mut roots = Vec::new();
    for base in runtime_root_candidates() {
        if !roots.iter().any(|existing| existing == &base) {
            roots.push(base.clone());
        }
        for root in candidate_texture_search_roots_from_base(&base) {
            if !roots.iter().any(|existing| existing == &root) {
                roots.push(root);
            }
        }
    }
    roots
}

fn startup_shell_fallback_resource_names(name: &str, filename: &str) -> Vec<String> {
    let mut with_explicit_english_texture_path = |file: &str, out: &mut Vec<String>| {
        let explicit = format!("Data/English/Art/Textures/{file}");
        if !out.iter().any(|existing| existing == &explicit) {
            out.push(explicit);
        }
        for candidate in candidate_texture_resource_names(file) {
            if !out.iter().any(|existing| existing == &candidate) {
                out.push(candidate);
            }
        }
    };

    if name.eq_ignore_ascii_case("MainMenuBackdrop")
        && (filename.eq_ignore_ascii_case("MainMenuBackdropuserinterface.tga")
            || filename.eq_ignore_ascii_case("MainMenuBackdrop.tga"))
    {
        // C++-compat fallback chain: prefer the authored MainMenuBackdrop payload
        // when present, but allow TitleScreen as the known stock fallback in
        // trimmed asset sets.
        let mut fallback = Vec::new();
        with_explicit_english_texture_path("MainMenuBackdropuserinterface.tga", &mut fallback);
        with_explicit_english_texture_path("MainMenuBackdrop.tga", &mut fallback);
        with_explicit_english_texture_path("TitleScreenuserinterface.tga", &mut fallback);
        return fallback;
    }

    if name.eq_ignore_ascii_case("MainMenuPulse")
        && filename.eq_ignore_ascii_case("MainMenuPulseuserinterface.tga")
    {
        let mut fallback = Vec::new();
        with_explicit_english_texture_path("SCShellUserInterface512_001.tga", &mut fallback);
        with_explicit_english_texture_path("SCSmShellUserInterface512_001.tga", &mut fallback);
        return fallback;
    }

    if name.eq_ignore_ascii_case("GeneralsLogo")
        && filename.eq_ignore_ascii_case("GeneralsLogouserinterface.tga")
    {
        let mut fallback = Vec::new();
        with_explicit_english_texture_path("SCSmShellUserInterface512_001.tga", &mut fallback);
        with_explicit_english_texture_path("SCShellUserInterface512_001.tga", &mut fallback);
        return fallback;
    }

    if name.eq_ignore_ascii_case("MainMenuRuler")
        && filename.eq_ignore_ascii_case("MainMenuRuleruserinterface.tga")
    {
        let mut fallback = vec!["Art/Textures/MainMenuRuleruserinterface.tga".to_string()];
        for candidate in candidate_texture_resource_names("MainMenuRuleruserinterface.tga") {
            if !fallback.iter().any(|existing| existing == &candidate) {
                fallback.push(candidate);
            }
        }
        return fallback;
    }

    Vec::new()
}

fn try_load_image_from_engine_filesystem(resource_name: &str) -> Option<DynamicImage> {
    ensure_engine_filesystem_backends();

    let fs = get_file_system();
    let bytes = {
        let mut fs_guard = fs.lock().ok()?;
        let mut file = fs_guard.open_file(resource_name, FileAccess::READ)?;
        file.read_entire_and_close().ok()?
    };

    decode_image_from_bytes(resource_name, &bytes).ok()
}

fn decode_image_from_bytes(resource_name: &str, bytes: &[u8]) -> Result<DynamicImage, ImageError> {
    let extension = Path::new(resource_name)
        .extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| ext.to_ascii_lowercase());

    let decode_by_extension = match extension.as_deref() {
        Some("tga") => image::load_from_memory_with_format(bytes, ImageFormat::Tga),
        Some("dds") => image::load_from_memory_with_format(bytes, ImageFormat::Dds),
        Some("png") => image::load_from_memory_with_format(bytes, ImageFormat::Png),
        Some("jpg") | Some("jpeg") => image::load_from_memory_with_format(bytes, ImageFormat::Jpeg),
        Some("bmp") => image::load_from_memory_with_format(bytes, ImageFormat::Bmp),
        _ => image::load_from_memory(bytes),
    };

    if decode_by_extension.is_ok() {
        return decode_by_extension;
    }

    for format in [
        ImageFormat::Dds,
        ImageFormat::Tga,
        ImageFormat::Png,
        ImageFormat::Jpeg,
        ImageFormat::Bmp,
    ] {
        if let Ok(image) = image::load_from_memory_with_format(bytes, format) {
            return Ok(image);
        }
    }

    image::load_from_memory(bytes)
}

fn ensure_engine_filesystem_backends() {
    let fs = get_file_system();
    let Ok(mut fs_guard) = fs.lock() else {
        return;
    };

    let writable = {
        let data = global_data::read();
        data.writable.clone()
    };

    let mut search_paths = vec![
        PathBuf::from("."),
        PathBuf::from("Data"),
        PathBuf::from("Art"),
        PathBuf::from("English"),
        PathBuf::from("Maps"),
        PathBuf::from("Assets"),
        PathBuf::from("Mods"),
        PathBuf::from("windows_game"),
        PathBuf::from("windows_game/Command & Conquer Generals Zero Hour"),
        PathBuf::from("windows_game/extracted_big_files"),
        PathBuf::from("windows_game/extracted_big_files/EnglishZH"),
        PathBuf::from("windows_game/extracted_big_files/TexturesZH"),
        PathBuf::from("windows_game/extracted_big_files_v2"),
        PathBuf::from("windows_game/extracted_big_files_v2/EnglishZH"),
        PathBuf::from("windows_game/extracted_big_files_v2/TexturesZH"),
        PathBuf::from("GeneralsRust/Code/Main/assets"),
    ];

    for base in runtime_root_candidates() {
        search_paths.push(base.clone());
        search_paths.push(base.join("Data"));
        search_paths.push(base.join("Art"));
        search_paths.push(base.join("English"));
        search_paths.push(base.join("Maps"));
        search_paths.push(base.join("Assets"));
        search_paths.push(base.join("Mods"));
        search_paths.push(base.join("windows_game"));
        search_paths.push(base.join("windows_game/Command & Conquer Generals Zero Hour"));
        search_paths.push(base.join("windows_game/extracted_big_files"));
        search_paths.push(base.join("windows_game/extracted_big_files/EnglishZH"));
        search_paths.push(base.join("windows_game/extracted_big_files/TexturesZH"));
        search_paths.push(base.join("windows_game/extracted_big_files_v2"));
        search_paths.push(base.join("windows_game/extracted_big_files_v2/EnglishZH"));
        search_paths.push(base.join("windows_game/extracted_big_files_v2/TexturesZH"));
        for ancestor in base.ancestors() {
            search_paths.push(ancestor.join("windows_game"));
            search_paths.push(ancestor.join("windows_game/Command & Conquer Generals Zero Hour"));
            search_paths.push(ancestor.join("windows_game/extracted_big_files"));
            search_paths.push(ancestor.join("windows_game/extracted_big_files/EnglishZH"));
            search_paths.push(ancestor.join("windows_game/extracted_big_files/TexturesZH"));
            search_paths.push(ancestor.join("windows_game/extracted_big_files_v2"));
            search_paths.push(ancestor.join("windows_game/extracted_big_files_v2/EnglishZH"));
            search_paths.push(ancestor.join("windows_game/extracted_big_files_v2/TexturesZH"));
        }
    }

    if !writable.mod_dir.is_empty() {
        search_paths.push(PathBuf::from(&writable.mod_dir));
    }
    if !writable.mod_big.is_empty() {
        if let Some(parent) = Path::new(&writable.mod_big).parent() {
            search_paths.push(parent.to_path_buf());
        }
    }

    {
        let local_backend: &mut LocalFileSystem = fs_guard.ensure_backend(LocalFileSystem::new);
        for path in &search_paths {
            local_backend.add_search_path(path);
        }
    }

    {
        let big_backend: &mut BigArchiveBackend = fs_guard.ensure_backend(BigArchiveBackend::new);
        for path in &search_paths {
            big_backend.add_search_path(path);
        }
    }

    fs_guard.clear_cache();

    if fs_guard.state() != SubsystemState::Running {
        let _ = CommonSubsystemInterface::init(&mut *fs_guard);
    }
}

/// Import mapped image metadata from the common INI collection into the
/// client-side collection used by GUI and renderer systems.
pub fn sync_mapped_images_from_common() -> usize {
    let Some(common_collection) = get_common_mapped_image_collection() else {
        return 0;
    };

    let common = common_collection.read();
    let total = common.len();
    if total == 0 {
        return 0;
    }

    let client_collection = ensure_mapped_image_collection();
    let mut client = client_collection.write();
    let mut imported = 0usize;

    for index in 0..total {
        let Some(common_image) = common.enum_image(index) else {
            continue;
        };

        let mut image = Image::new();
        image.set_name(common_image.get_name().to_string());
        image.set_filename(common_image.get_filename().to_string());
        image.set_texture_width(common_image.get_texture_size().x);
        image.set_texture_height(common_image.get_texture_size().y);
        image.set_uv(Region2D::from_coords(
            common_image.get_uv().left,
            common_image.get_uv().top,
            common_image.get_uv().right,
            common_image.get_uv().bottom,
        ));
        image.set_image_size(ICoord2D::new(
            common_image.get_image_size().x,
            common_image.get_image_size().y,
        ));
        if common_image.status.is_rotated_90_clockwise() {
            image.set_status(ImageStatus::ROTATED_90_CLOCKWISE);
        }

        client.add_image(image);
        imported += 1;
    }

    imported
}

fn import_common_mapped_image_into_client(name: &str) -> bool {
    CommonImageCollection::load_global(512);
    let Some(common_collection) = get_common_mapped_image_collection() else {
        return false;
    };
    if common_collection.read().is_empty() {
        CommonImageCollection::load_global(512);
    }

    let common = common_collection.read();
    let Some(common_image) = common.find_image_by_name(name) else {
        if is_startup_shell_image(name) {
            log_startup_shell_image_once(name, format!("common_missing total={}", common.len()));
        }
        return false;
    };

    let client_collection = ensure_mapped_image_collection();
    let mut client = client_collection.write();
    if client.find_image_by_name(name).is_some() {
        return true;
    }

    let mut image = Image::new();
    image.set_name(common_image.get_name().to_string());
    image.set_filename(common_image.get_filename().to_string());
    image.set_texture_width(common_image.get_texture_size().x);
    image.set_texture_height(common_image.get_texture_size().y);
    image.set_uv(Region2D::from_coords(
        common_image.get_uv().left,
        common_image.get_uv().top,
        common_image.get_uv().right,
        common_image.get_uv().bottom,
    ));
    image.set_image_size(ICoord2D::new(
        common_image.get_image_size().x,
        common_image.get_image_size().y,
    ));
    if common_image.status.is_rotated_90_clockwise() {
        image.set_status(ImageStatus::ROTATED_90_CLOCKWISE);
    }

    client.add_image(image);
    if is_startup_shell_image(name) {
        log_startup_shell_image_once(name, "hydrated_from_common".to_string());
    }
    true
}

pub fn ensure_client_mapped_image(name: &str) -> bool {
    {
        let client = ensure_mapped_image_collection();
        if client.read().find_image_by_name(name).is_some() {
            return true;
        }
    }

    import_common_mapped_image_into_client(name)
}

/// Get the global mapped image collection
pub fn get_mapped_image_collection() -> Arc<RwLock<ImageCollection>> {
    ensure_mapped_image_collection()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_image_creation() {
        let image = Image::new();
        assert_eq!(image.get_name(), "");
        assert_eq!(image.get_image_width(), 0);
        assert_eq!(image.get_image_height(), 0);
        assert_eq!(image.get_status(), ImageStatus::NONE);
    }

    #[test]
    fn test_image_with_name() {
        let image = Image::with_name("test_image");
        assert_eq!(image.get_name(), "test_image");
    }

    #[test]
    fn test_image_status() {
        let mut image = Image::new();
        assert!(!image.has_alpha());

        image.set_status(ImageStatus::HAS_ALPHA);
        assert!(image.has_alpha());
        assert!(image.has_status(ImageStatus::HAS_ALPHA));

        image.clear_status(ImageStatus::HAS_ALPHA);
        assert!(!image.has_alpha());
    }

    #[test]
    fn test_region2d() {
        let region = Region2D::new(10.0, 20.0, 100.0, 200.0);
        assert_eq!(region.width(), 100.0);
        assert_eq!(region.height(), 200.0);

        let normalized = Region2D::normalized_from_pixel_coords(0, 0, 256, 256, 512, 512);
        assert_eq!(normalized.min.x, 0.0);
        assert_eq!(normalized.min.y, 0.0);
        assert_eq!(normalized.max.x, 0.5);
        assert_eq!(normalized.max.y, 0.5);
    }

    #[test]
    fn test_icoord2d() {
        let coord = ICoord2D::new(100, 200);
        assert_eq!(coord.x, 100);
        assert_eq!(coord.y, 200);

        let zero = ICoord2D::ZERO;
        assert_eq!(zero.x, 0);
        assert_eq!(zero.y, 0);
    }

    #[test]
    fn test_image_collection() {
        let mut collection = ImageCollection::new();
        assert_eq!(collection.count(), 0);

        let image = Image::with_name("test");
        collection.add_image(image);
        assert_eq!(collection.count(), 1);

        assert!(collection.find_image_by_name("test").is_some());
        assert!(collection.find_image_by_name("Test").is_some()); // Case insensitive
        assert!(collection.find_image_by_name("nonexistent").is_none());
    }

    #[test]
    fn test_image_coords() {
        let mut image = Image::new();
        image.set_texture_width(512);
        image.set_texture_height(512);
        image.set_coords(0, 0, 256, 256);

        assert_eq!(image.get_image_width(), 256);
        assert_eq!(image.get_image_height(), 256);

        let uv = image.get_uv();
        assert_eq!(uv.min.x, 0.0);
        assert_eq!(uv.min.y, 0.0);
        assert_eq!(uv.max.x, 0.5);
        assert_eq!(uv.max.y, 0.5);
    }

    #[test]
    fn sync_mapped_images_from_common_imports_shell_menu_images() {
        game_engine::common::ini::ini_mapped_image::init_global_mapped_image_collection();
        CommonImageCollection::load_global(512);

        let client = ensure_mapped_image_collection();
        client.write().clear();

        let imported = sync_mapped_images_from_common();
        assert!(imported > 0, "expected mapped images to import");

        let client = client.read();
        for name in [
            "MainMenuBackdrop",
            "MainMenuPulse",
            "MainMenuRuler",
            "GeneralsLogo",
        ] {
            assert!(
                client.find_image_by_name(name).is_some(),
                "{name} missing after mapped image sync; imported={imported} total={}",
                client.count()
            );
        }
    }

    #[test]
    fn startup_shell_fallback_resource_names_prefers_real_shipped_shell_assets() {
        let backdrop = startup_shell_fallback_resource_names(
            "MainMenuBackdrop",
            "MainMenuBackdropuserinterface.tga",
        );
        assert!(backdrop.iter().any(|v| v.ends_with("MainMenuBackdrop.tga")));
        assert!(backdrop
            .iter()
            .any(|v| v.ends_with("Data/English/Art/Textures/MainMenuBackdrop.tga")));
        assert!(backdrop
            .iter()
            .any(|v| v.ends_with("TitleScreenuserinterface.tga")));

        let logo =
            startup_shell_fallback_resource_names("GeneralsLogo", "GeneralsLogouserinterface.tga");
        assert!(logo
            .iter()
            .any(|v| v.ends_with("SCSmShellUserInterface512_001.tga")));

        let ruler = startup_shell_fallback_resource_names(
            "MainMenuRuler",
            "MainMenuRuleruserinterface.tga",
        );
        assert!(ruler
            .iter()
            .any(|v| v.ends_with("Art/Textures/MainMenuRuleruserinterface.tga")));
    }

    #[test]
    fn black_square_synthesizes_when_no_backing_asset_exists() {
        let mut image = Image::with_name("BlackSquare");
        image.set_filename("DefinitelyMissingBlackSquareAsset.tga".to_string());

        image
            .ensure_image_data_loaded()
            .expect("BlackSquare should synthesize a fallback texture");

        assert_eq!(image.get_image_width(), 1);
        assert_eq!(image.get_image_height(), 1);
        assert!(image.has_alpha());
    }

    #[test]
    fn resolve_case_insensitive_path_matches_mixed_case_asset_paths() {
        let temp_dir = TempDir::new().expect("temp dir");
        let asset_path = temp_dir
            .path()
            .join("Art")
            .join("Textures")
            .join("mainmenuruleruserinterface.tga");
        fs::create_dir_all(asset_path.parent().expect("asset parent")).expect("create dirs");
        fs::write(&asset_path, b"test").expect("write asset");

        let requested = temp_dir
            .path()
            .join("art")
            .join("textures")
            .join("MainMenuRuleruserinterface.tga");
        let resolved =
            resolve_case_insensitive_path(&requested).expect("case-insensitive path resolution");

        assert_eq!(resolved, asset_path);
    }
}
