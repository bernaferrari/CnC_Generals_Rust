//! Texture Loader System
//!
//! This module provides texture loading and management functionality,
//! equivalent to the original texture loading system.

use crate::core::error::{Error, RendererResult};
use crate::core::ww3dformat::{FormatManager, WW3DFormat};
use crate::rendering::texture_decode::{
    decode_texture_file, TextureData, TextureDataKind, TextureMipLevel,
};
use crate::rendering::texture_quality;
use crate::rendering::texture_system::texture_base::{
    PoolType, TextureBaseClass, TextureUsagePolicy,
};
use log::warn;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use wgpu::{Device, Queue};

/// Texture loading priority
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum TextureLoadPriority {
    Low,
    Medium,
    High,
    Critical,
}

#[derive(Debug, Clone)]
struct PendingTextureLoad {
    filename: String,
    priority: TextureLoadPriority,
}

/// Texture loader class
pub struct TextureLoader {
    device: Arc<Device>,
    queue: Arc<Queue>,
    loaded_textures: HashMap<String, Box<TextureBaseClass>>,
    pending_loads: Vec<PendingTextureLoad>,
    format_manager: Arc<FormatManager>,
}

impl TextureLoader {
    /// Create new texture loader
    pub fn new(device: Arc<Device>, queue: Arc<Queue>) -> RendererResult<Self> {
        let format_manager = Arc::new(FormatManager::from_device(&device));

        Ok(Self {
            device,
            queue,
            loaded_textures: HashMap::new(),
            pending_loads: Vec::new(),
            format_manager,
        })
    }

    /// Load texture from file
    pub fn load_texture(
        &mut self,
        filename: &str,
        _format: wgpu::TextureFormat,
        _mip_count: u32,
        pool: PoolType,
    ) -> RendererResult<&TextureBaseClass> {
        // Check if already loaded
        if self.loaded_textures.contains_key(filename) {
            return Ok(self.loaded_textures.get(filename).unwrap());
        }

        // Load texture from file
        let mut texture = self.load_texture_from_file_internal(filename, pool)?;
        texture.set_name(filename);
        texture.set_full_path(filename);

        self.loaded_textures
            .insert(filename.to_string(), Box::new(texture));

        // Get the texture after insertion to avoid borrowing conflict
        match self.loaded_textures.get(filename) {
            Some(texture) => Ok(texture),
            None => panic!("Texture was just inserted but not found"), // Should never happen
        }
    }

    /// Unload texture
    pub fn unload_texture(&mut self, filename: &str) -> bool {
        self.loaded_textures.remove(filename).is_some()
    }

    /// Get loaded texture
    pub fn get_texture(&self, filename: &str) -> Option<&TextureBaseClass> {
        self.loaded_textures.get(filename).map(|t| t.as_ref())
    }

    /// Check if texture is loaded
    pub fn is_texture_loaded(&self, filename: &str) -> bool {
        self.loaded_textures.contains_key(filename)
    }

    /// Get memory usage
    pub fn memory_usage(&self) -> usize {
        self.loaded_textures.len() * 512 * 512 * 4 // Rough estimate
    }

    /// Flush pending loads
    pub fn flush_pending_loads(&mut self) {
        self.pending_loads.clear();
    }

    /// Get device reference
    pub fn device(&self) -> &Arc<Device> {
        &self.device
    }

    /// Get queue reference
    pub fn queue(&self) -> &Arc<Queue> {
        &self.queue
    }

    /// Add to pending loads
    pub fn add_pending_load(&mut self, filename: &str, priority: TextureLoadPriority) {
        Self::upsert_pending_load(&mut self.pending_loads, filename, priority);
    }

    /// Process pending loads
    pub fn process_pending_loads(&mut self) {
        Self::sort_pending_loads(&mut self.pending_loads);

        let pending = std::mem::take(&mut self.pending_loads);
        for entry in pending {
            if self.loaded_textures.contains_key(&entry.filename) {
                continue;
            }

            match self.load_texture_from_file_internal(&entry.filename, PoolType::Managed) {
                Ok(mut texture) => {
                    texture.set_name(&entry.filename);
                    texture.set_full_path(&entry.filename);
                    self.loaded_textures
                        .insert(entry.filename.clone(), Box::new(texture));
                }
                Err(err) => {
                    warn!("Failed pending texture load '{}': {}", entry.filename, err);
                }
            }
        }
    }

    fn upsert_pending_load(
        queue: &mut Vec<PendingTextureLoad>,
        filename: &str,
        priority: TextureLoadPriority,
    ) {
        if let Some(existing) = queue
            .iter_mut()
            .find(|entry| entry.filename.eq_ignore_ascii_case(filename))
        {
            if priority > existing.priority {
                existing.priority = priority;
            }
            return;
        }

        queue.push(PendingTextureLoad {
            filename: filename.to_string(),
            priority,
        });
    }

    fn sort_pending_loads(queue: &mut [PendingTextureLoad]) {
        queue.sort_by(|a, b| {
            b.priority
                .cmp(&a.priority)
                .then_with(|| a.filename.cmp(&b.filename))
        });
    }

    /// Load texture from file path (for ModernTextureManager compatibility)
    pub fn load_texture_from_path(&mut self, path: &Path) -> RendererResult<TextureBaseClass> {
        let filename = path.to_string_lossy().to_string();

        // Check if already loaded
        if let Some(texture) = self.loaded_textures.get(&filename) {
            return Ok((**texture).clone());
        }

        // Load texture from file
        let mut texture = self.load_texture_from_file_internal(&filename, PoolType::Managed)?;
        texture.set_name(&filename);
        texture.set_full_path(&filename);

        let result = texture.clone();
        self.loaded_textures.insert(filename, Box::new(texture));
        Ok(result)
    }

    /// Internal method to load texture from file
    fn load_texture_from_file_internal(
        &self,
        filename: &str,
        pool: PoolType,
    ) -> RendererResult<TextureBaseClass> {
        let path = Path::new(filename);
        let mut decode_error: Option<Error> = None;
        for candidate in self.get_possible_texture_paths(path) {
            if candidate.exists() {
                match decode_texture_file(&candidate) {
                    Ok(mut data) => {
                        let asset_type = match data.kind {
                            TextureDataKind::Texture2D => crate::rendering::texture_system::texture_base::TexAssetType::Regular,
                            TextureDataKind::CubeMap => crate::rendering::texture_system::texture_base::TexAssetType::Cubemap,
                            TextureDataKind::Volume => crate::rendering::texture_system::texture_base::TexAssetType::Volume,
                        };

                        let allow_compression = true;
                        let decision =
                            self.format_manager
                                .decide(data.format, None, allow_compression);
                        data = data.convert_to_format(&decision)?;

                        if matches!(data.kind, TextureDataKind::Texture2D) {
                            let reduction = texture_quality::compute_effective_reduction(
                                data.width,
                                data.height,
                                data.mip_levels,
                            );
                            if reduction > 0 {
                                data = data.drop_mip_levels(reduction);
                            }
                        }

                        let full_path = candidate.to_string_lossy().to_string();
                        let mut texture_base = TextureBaseClass::new(
                            data.width,
                            data.height,
                            data.mip_levels,
                            pool,
                            asset_type,
                        );
                        texture_base.apply_texture_data(&data);
                        texture_base.set_usage_policy(TextureUsagePolicy::default());
                        texture_base.ensure_gpu_texture(&self.device, &self.queue)?;
                        texture_base.set_full_path(&full_path);
                        return Ok(texture_base);
                    }
                    Err(err) => {
                        decode_error = Some(err);
                    }
                }
            }
        }

        if let Some(err) = decode_error {
            return Err(err);
        }

        Err(Error::FileNotFound(format!(
            "Texture not found on filesystem: {}",
            filename
        )))
    }
    fn get_possible_texture_paths(&self, path: &Path) -> Vec<PathBuf> {
        let mut paths = Vec::new();
        paths.push(path.to_path_buf());

        if path.extension().is_none() {
            let stem = path.file_stem().unwrap_or_default();
            let parent = path.parent().unwrap_or(Path::new(""));
            paths.push(parent.join(format!("{}.dds", stem.to_string_lossy())));
            paths.push(parent.join(format!("{}.tga", stem.to_string_lossy())));
            paths.push(parent.join(format!("{}.png", stem.to_string_lossy())));
            paths.push(parent.join(format!("{}.jpg", stem.to_string_lossy())));
        } else {
            let stem = path.file_stem().unwrap_or_default();
            let parent = path.parent().unwrap_or(Path::new(""));
            if path.extension() != Some(std::ffi::OsStr::new("dds")) {
                paths.push(parent.join(format!("{}.dds", stem.to_string_lossy())));
            }
            if path.extension() != Some(std::ffi::OsStr::new("tga")) {
                paths.push(parent.join(format!("{}.tga", stem.to_string_lossy())));
            }
            if path.extension() != Some(std::ffi::OsStr::new("png")) {
                paths.push(parent.join(format!("{}.png", stem.to_string_lossy())));
            }
            if path.extension() != Some(std::ffi::OsStr::new("jpg")) {
                paths.push(parent.join(format!("{}.jpg", stem.to_string_lossy())));
            }
        }

        paths
    }

    /// Create a checkerboard fallback texture.
    pub fn create_missing_texture(
        &self,
        filename: &str,
        pool: PoolType,
    ) -> RendererResult<TextureBaseClass> {
        let width = 2;
        let height = 2;
        let data = vec![
            255, 0, 255, 255, 0, 0, 0, 255, 0, 0, 0, 255, 255, 0, 255, 255,
        ];

        let texture_data = TextureData {
            width,
            height,
            depth: 1,
            mip_levels: 1,
            format: WW3DFormat::A8R8G8B8,
            kind: TextureDataKind::Texture2D,
            data,
            mip_layout: vec![TextureMipLevel {
                offset: 0,
                size: 4 * width as usize * height as usize,
                width,
                height,
                depth_or_layers: 1,
                slice_stride: 4 * width as usize * height as usize,
            }],
            format_decision: None,
        };

        let mut texture_base = TextureBaseClass::new(
            width,
            height,
            1,
            pool,
            crate::rendering::texture_system::texture_base::TexAssetType::Regular,
        );
        texture_base.apply_texture_data(&texture_data);
        texture_base.ensure_gpu_texture(&self.device, &self.queue)?;
        texture_base.set_name(filename);
        texture_base.set_full_path(filename);
        Ok(texture_base)
    }
}

#[cfg(test)]
mod tests {
    use super::{PendingTextureLoad, TextureLoadPriority, TextureLoader};

    #[test]
    fn pending_queue_upsert_promotes_priority_without_duplicates() {
        let mut queue = Vec::new();
        TextureLoader::upsert_pending_load(&mut queue, "grass", TextureLoadPriority::Low);
        TextureLoader::upsert_pending_load(&mut queue, "grass", TextureLoadPriority::Critical);
        TextureLoader::upsert_pending_load(&mut queue, "GRASS", TextureLoadPriority::Medium);

        assert_eq!(queue.len(), 1);
        assert_eq!(queue[0].filename, "grass");
        assert_eq!(queue[0].priority, TextureLoadPriority::Critical);
    }

    #[test]
    fn pending_queue_sort_orders_high_to_low() {
        let mut queue = vec![
            PendingTextureLoad {
                filename: "c".to_string(),
                priority: TextureLoadPriority::Low,
            },
            PendingTextureLoad {
                filename: "a".to_string(),
                priority: TextureLoadPriority::Critical,
            },
            PendingTextureLoad {
                filename: "b".to_string(),
                priority: TextureLoadPriority::High,
            },
        ];

        TextureLoader::sort_pending_loads(&mut queue);

        let priorities: Vec<TextureLoadPriority> =
            queue.iter().map(|entry| entry.priority).collect();
        assert_eq!(
            priorities,
            vec![
                TextureLoadPriority::Critical,
                TextureLoadPriority::High,
                TextureLoadPriority::Low
            ]
        );
    }
}
