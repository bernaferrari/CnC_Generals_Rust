/*
**  Command & Conquer Generals Zero Hour™
*/

//! Minimal texture manager built on top of the shared `wgpu` graphics context.

use crate::platform::GraphicsContext;
use std::collections::HashMap;
use std::path::Path;
use std::sync::{
    atomic::{AtomicU64, Ordering},
    Arc, Mutex,
};
use thiserror::Error;

/// Texture system error types
#[derive(Error, Debug)]
pub enum TextureError {
    #[error("Texture loading failed: {0}")]
    LoadingFailed(String),
    #[error("Texture creation failed: {0}")]
    CreationFailed(String),
}

/// Texture handle for efficient referencing
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TextureHandle(u64);

impl TextureHandle {
    pub fn invalid() -> Self {
        Self(0)
    }

    pub fn is_valid(self) -> bool {
        self.0 != 0
    }
}

/// Descriptor used when creating a texture from raw data.
#[derive(Debug, Clone)]
pub struct TextureDescriptor {
    pub label: String,
    pub size: (u32, u32),
    pub format: wgpu::TextureFormat,
}

impl Default for TextureDescriptor {
    fn default() -> Self {
        Self {
            label: "texture".to_string(),
            size: (1, 1),
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
        }
    }
}

struct StoredTexture {
    texture: wgpu::Texture,
    view: Arc<wgpu::TextureView>,
}

/// Lightweight texture manager that owns textures uploaded to the GPU.
pub struct TextureManager {
    context: GraphicsContext,
    textures: Mutex<HashMap<TextureHandle, Arc<StoredTexture>>>, // store texture + view
    label_index: Mutex<HashMap<String, TextureHandle>>,
    next_handle: AtomicU64,
    memory_budget: u64,
    memory_used: AtomicU64,
}

impl TextureManager {
    pub fn new(context: GraphicsContext, memory_budget: u64) -> Result<Self, TextureError> {
        Ok(Self {
            context,
            textures: Mutex::new(HashMap::new()),
            label_index: Mutex::new(HashMap::new()),
            next_handle: AtomicU64::new(1),
            memory_budget,
            memory_used: AtomicU64::new(0),
        })
    }

    fn alloc_handle(&self) -> TextureHandle {
        TextureHandle(self.next_handle.fetch_add(1, Ordering::Relaxed))
    }

    pub async fn initialize_defaults(&mut self) -> Result<(), TextureError> {
        // No default textures yet – future work will upload them here.
        Ok(())
    }

    pub async fn create_solid_texture(
        &self,
        label: &str,
        color: [u8; 4],
    ) -> Result<TextureHandle, TextureError> {
        let data = vec![color[0], color[1], color[2], color[3]];
        self.create_texture_from_rgba(label, 1, 1, &data)
    }

    pub fn create_texture_from_rgba(
        &self,
        label: &str,
        width: u32,
        height: u32,
        rgba: &[u8],
    ) -> Result<TextureHandle, TextureError> {
        if (width as usize) * (height as usize) * 4 != rgba.len() {
            return Err(TextureError::LoadingFailed(format!(
                "{} has invalid RGBA data length",
                label
            )));
        }

        let texture = self
            .context
            .device()
            .create_texture(&wgpu::TextureDescriptor {
                label: Some(label),
                size: wgpu::Extent3d {
                    width,
                    height,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Rgba8UnormSrgb,
                usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
                view_formats: &[],
            });

        self.context.queue().write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            rgba,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(4 * width),
                rows_per_image: Some(height),
            },
            wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
        );

        let view = Arc::new(texture.create_view(&wgpu::TextureViewDescriptor::default()));
        let handle = self.alloc_handle();
        let stored = Arc::new(StoredTexture {
            texture,
            view: Arc::clone(&view),
        });
        self.textures.lock().unwrap_or_else(|e| e.into_inner()).insert(handle, stored);
        self.label_index.lock().unwrap_or_else(|e| e.into_inner())
            .insert(label.to_ascii_lowercase(), handle);
        self.memory_used
            .fetch_add((width as u64) * (height as u64) * 4, Ordering::Relaxed);

        Ok(handle)
    }

    pub fn get_handle_by_label(&self, label: &str) -> Option<TextureHandle> {
        self.label_index.lock().unwrap_or_else(|e| e.into_inner())
            .get(&label.to_ascii_lowercase())
            .copied()
    }

    pub fn get_view(&self, handle: TextureHandle) -> Option<Arc<wgpu::TextureView>> {
        self.textures.lock().unwrap_or_else(|e| e.into_inner())
            .get(&handle)
            .map(|stored| Arc::clone(&stored.view))
    }

    pub async fn load_texture_from_path(&self, path: &Path) -> Result<TextureHandle, TextureError> {
        let data = std::fs::read(path).map_err(|e| TextureError::LoadingFailed(e.to_string()))?;
        let extension = path
            .extension()
            .and_then(|ext| ext.to_str())
            .unwrap_or("")
            .to_lowercase();
        let image = match extension.as_str() {
            "tga" => image::load_from_memory_with_format(&data, image::ImageFormat::Tga),
            "dds" => image::load_from_memory_with_format(&data, image::ImageFormat::Dds),
            _ => image::load_from_memory(&data),
        }
        .map_err(|e| TextureError::LoadingFailed(e.to_string()))?
        .to_rgba8();
        let (width, height) = image.dimensions();
        self.create_texture_from_rgba(path.to_string_lossy().as_ref(), width, height, &image)
    }

    pub fn get_texture_view(&self, handle: TextureHandle) -> Option<Arc<wgpu::TextureView>> {
        self.textures.lock().unwrap_or_else(|e| e.into_inner())
            .get(&handle)
            .map(|stored| Arc::clone(&stored.view))
    }
}
