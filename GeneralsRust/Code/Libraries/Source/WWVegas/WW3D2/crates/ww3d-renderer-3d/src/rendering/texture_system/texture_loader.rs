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
use ww3d_core::W3dTextureStruct;

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

    /// Load a texture referenced by a W3D texture descriptor.
    ///
    /// C++ `Load_Texture` reads the texture filename, derives mip count and
    /// format from `W3DTEXTURE_*`, loads through the asset manager, then applies
    /// U/V address modes from the same flags.
    pub fn load_w3d_descriptor(
        &mut self,
        descriptor: &W3dTextureStruct,
        pool: PoolType,
    ) -> RendererResult<TextureBaseClass> {
        let name = w3d_texture_name(descriptor)?;
        let request = texture_request_from_w3d_descriptor(descriptor);
        let cache_key = w3d_texture_cache_key(&name, &request);

        if let Some(texture) = self.loaded_textures.get(&cache_key) {
            return Ok((**texture).clone());
        }

        let mut texture =
            self.load_texture_from_file_internal_with_request(&name, pool, &request)?;
        texture.set_name(&name);
        texture.set_full_path(&name);
        texture.set_u_address_mode(request.address_u);
        texture.set_v_address_mode(request.address_v);
        texture.set_usage_policy(TextureUsagePolicy::new(
            true,
            request.allow_reduction,
            request.requested_mip_levels,
        ));

        let result = texture.clone();
        self.loaded_textures.insert(cache_key, Box::new(texture));
        Ok(result)
    }

    /// Internal method to load texture from file
    fn load_texture_from_file_internal(
        &self,
        filename: &str,
        pool: PoolType,
    ) -> RendererResult<TextureBaseClass> {
        self.load_texture_from_file_internal_with_request(
            filename,
            pool,
            &TextureDescriptorRequest::default(),
        )
    }

    fn load_texture_from_file_internal_with_request(
        &self,
        filename: &str,
        pool: PoolType,
        request: &TextureDescriptorRequest,
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
                        let decision = self.format_manager.decide(
                            data.format,
                            request.desired_format,
                            allow_compression,
                        );
                        data = data.convert_to_format(&decision)?;

                        if let Some(target) = request.requested_mip_levels {
                            if target > data.mip_levels && request.generate_mipmaps {
                                if let Err(err) = data.ensure_mip_levels(target) {
                                    warn!("Failed to generate mipmaps for {}: {}", filename, err);
                                }
                            }
                        } else if request.generate_mipmaps {
                            let desired = data.max_possible_mip_levels();
                            if desired > data.mip_levels {
                                if let Err(err) = data.ensure_mip_levels(desired) {
                                    warn!(
                                        "Failed to generate full mip chain for {}: {}",
                                        filename, err
                                    );
                                }
                            }
                        }

                        if matches!(data.kind, TextureDataKind::Texture2D)
                            && request.allow_reduction
                        {
                            let mut reduction = texture_quality::compute_effective_reduction(
                                data.width,
                                data.height,
                                data.mip_levels,
                            );
                            if let Some(target) = request.requested_mip_levels {
                                let max_drop = data.mip_levels.saturating_sub(target.max(1));
                                reduction = reduction.min(max_drop);
                            }
                            if reduction > 0 {
                                data = data.drop_mip_levels(reduction);
                            }
                        }

                        if let Some(target) = request.requested_mip_levels {
                            data = data.truncate_to_mip_count(target.max(1));
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
                        texture_base.set_usage_policy(TextureUsagePolicy::new(
                            true,
                            request.allow_reduction,
                            request.requested_mip_levels,
                        ));
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct TextureDescriptorRequest {
    requested_mip_levels: Option<u32>,
    generate_mipmaps: bool,
    allow_reduction: bool,
    desired_format: Option<WW3DFormat>,
    address_u: crate::rendering::texture_system::texture_base::TextureAddressMode,
    address_v: crate::rendering::texture_system::texture_base::TextureAddressMode,
}

impl Default for TextureDescriptorRequest {
    fn default() -> Self {
        Self {
            requested_mip_levels: None,
            generate_mipmaps: false,
            allow_reduction: true,
            desired_format: None,
            address_u: crate::rendering::texture_system::texture_base::TextureAddressMode::Wrap,
            address_v: crate::rendering::texture_system::texture_base::TextureAddressMode::Wrap,
        }
    }
}

fn w3d_texture_name(descriptor: &W3dTextureStruct) -> RendererResult<String> {
    let len = descriptor
        .name
        .iter()
        .position(|&byte| byte == 0)
        .unwrap_or(descriptor.name.len());
    let name = String::from_utf8_lossy(&descriptor.name[..len])
        .trim()
        .to_string();

    if name.is_empty() {
        return Err(Error::InvalidData(
            "W3D texture descriptor is missing a texture name".to_string(),
        ));
    }

    Ok(name)
}

fn texture_request_from_w3d_descriptor(descriptor: &W3dTextureStruct) -> TextureDescriptorRequest {
    use crate::rendering::texture_system::texture_base::TextureAddressMode;

    let attrs = descriptor.texture_info.attributes;
    let no_lod = attrs & ww3d_core::W3D_TEXTURE_NO_LOD != 0;
    let requested_mip_levels = if no_lod {
        Some(1)
    } else {
        match attrs & ww3d_core::W3D_TEXTURE_MIP_LEVELS_MASK {
            ww3d_core::W3D_TEXTURE_MIP_LEVELS_ALL => None,
            ww3d_core::W3D_TEXTURE_MIP_LEVELS_2 => Some(2),
            ww3d_core::W3D_TEXTURE_MIP_LEVELS_3 => Some(3),
            ww3d_core::W3D_TEXTURE_MIP_LEVELS_4 => Some(4),
            _ => None,
        }
    };

    let stage = crate::material_system::TextureStageSettings::from_descriptor(descriptor);
    let desired_format = if matches!(
        stage.texture_type,
        crate::material_system::TextureStageType::BumpMap
    ) {
        Some(WW3DFormat::U8V8)
    } else {
        None
    };

    TextureDescriptorRequest {
        requested_mip_levels,
        generate_mipmaps: !no_lod,
        allow_reduction: !no_lod,
        desired_format,
        address_u: if attrs & ww3d_core::W3D_TEXTURE_CLAMP_U != 0 {
            TextureAddressMode::Clamp
        } else {
            TextureAddressMode::Wrap
        },
        address_v: if attrs & ww3d_core::W3D_TEXTURE_CLAMP_V != 0 {
            TextureAddressMode::Clamp
        } else {
            TextureAddressMode::Wrap
        },
    }
}

fn w3d_texture_cache_key(name: &str, request: &TextureDescriptorRequest) -> String {
    format!(
        "{}|mips={:?}|fmt={:?}|u={:?}|v={:?}|reduce={}",
        name,
        request.requested_mip_levels,
        request.desired_format,
        request.address_u,
        request.address_v,
        request.allow_reduction
    )
}

#[cfg(test)]
mod tests {
    use super::{
        texture_request_from_w3d_descriptor, w3d_texture_cache_key, w3d_texture_name,
        PendingTextureLoad, TextureDescriptorRequest, TextureLoadPriority, TextureLoader,
    };
    use crate::core::WW3DFormat;
    use crate::rendering::texture_system::texture_base::TextureAddressMode;
    use ww3d_core::{W3dTextureInfoStruct, W3dTextureStruct};

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

    fn descriptor(name: &str, attrs: u16) -> W3dTextureStruct {
        let mut raw = [0u8; 256];
        raw[..name.len()].copy_from_slice(name.as_bytes());
        W3dTextureStruct {
            name: raw,
            texture_info: W3dTextureInfoStruct {
                attributes: attrs,
                animation_type: 0,
                frame_count: 0,
                frame_rate: 0.0,
            },
        }
    }

    #[test]
    fn w3d_descriptor_name_trims_c_string_like_cpp_chunk_name() {
        let tex = descriptor("terrain.tga", 0);
        assert_eq!(w3d_texture_name(&tex).unwrap(), "terrain.tga");
    }

    #[test]
    fn w3d_descriptor_flags_match_cpp_load_texture_mip_and_address_rules() {
        let tex = descriptor(
            "cliff.dds",
            ww3d_core::W3D_TEXTURE_MIP_LEVELS_3
                | ww3d_core::W3D_TEXTURE_CLAMP_U
                | ww3d_core::W3D_TEXTURE_CLAMP_V,
        );

        let request = texture_request_from_w3d_descriptor(&tex);

        assert_eq!(request.requested_mip_levels, Some(3));
        assert!(request.generate_mipmaps);
        assert!(request.allow_reduction);
        assert_eq!(request.address_u, TextureAddressMode::Clamp);
        assert_eq!(request.address_v, TextureAddressMode::Clamp);
        assert_eq!(request.desired_format, None);
    }

    #[test]
    fn w3d_no_lod_disables_mips_and_reduction_like_cpp_filter_none() {
        let tex = descriptor(
            "button",
            ww3d_core::W3D_TEXTURE_NO_LOD | ww3d_core::W3D_TEXTURE_MIP_LEVELS_4,
        );

        let request = texture_request_from_w3d_descriptor(&tex);

        assert_eq!(request.requested_mip_levels, Some(1));
        assert!(!request.generate_mipmaps);
        assert!(!request.allow_reduction);
    }

    #[test]
    fn w3d_bump_map_requests_bump_format_preference() {
        let tex = descriptor("normalmap", ww3d_core::W3D_TEXTURE_TYPE_BUMPMAP);

        let request = texture_request_from_w3d_descriptor(&tex);

        assert_eq!(request.desired_format, Some(WW3DFormat::U8V8));
    }

    #[test]
    fn w3d_texture_cache_key_separates_same_file_with_different_sampler_policy() {
        let base = TextureDescriptorRequest::default();
        let clamped = TextureDescriptorRequest {
            address_u: TextureAddressMode::Clamp,
            ..TextureDescriptorRequest::default()
        };

        assert_ne!(
            w3d_texture_cache_key("shared.dds", &base),
            w3d_texture_cache_key("shared.dds", &clamped)
        );
    }
}
