////////////////////////////////////////////////////////////////////////////////
//                                                                            //
//  (c) 2001-2003 Electronic Arts Inc.                                       //
//                                                                            //
////////////////////////////////////////////////////////////////////////////////

// Texture loading system for real C&C assets

use crate::assets::archive::ArchiveFileSystem;
use anyhow::{anyhow, Result};
use log::{debug, error, info, warn};
use std::collections::{HashMap, HashSet};
use ww3d_renderer_3d::rendering::texture_system::dds_loader::{
    decode_dxt1, decode_dxt3, decode_dxt5, load_dds_from_memory, DdsCompression, DdsTextureType,
};
use ww3d_renderer_3d::rendering::texture_system::tga_loader::load_tga_from_memory;

/// Texture formats supported by C&C Generals
#[derive(Debug, Clone, Copy)]
pub enum TextureFormat {
    TGA,
    DDS,
    BMP,
    JPG,
    PNG,
    Unknown,
}

impl TextureFormat {
    pub fn from_filename(filename: &str) -> Self {
        let filename_lower = filename.to_lowercase();
        if filename_lower.ends_with(".tga") {
            TextureFormat::TGA
        } else if filename_lower.ends_with(".dds") {
            TextureFormat::DDS
        } else if filename_lower.ends_with(".bmp") {
            TextureFormat::BMP
        } else if filename_lower.ends_with(".jpg") || filename_lower.ends_with(".jpeg") {
            TextureFormat::JPG
        } else if filename_lower.ends_with(".png") {
            TextureFormat::PNG
        } else {
            TextureFormat::Unknown
        }
    }
}

#[derive(Debug, Clone)]
pub struct RawTexture {
    pub name: String,
    pub width: u32,
    pub height: u32,
    pub data: Vec<u8>,
    pub format: TextureFormat,
    pub has_alpha: bool,
    pub dds_compression: Option<DdsCompression>,
}

#[derive(Debug, Clone)]
pub struct WaterTextureAssetPayload {
    pub name: String,
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}

#[derive(Debug, Clone)]
pub struct WaterTextureAssetSet {
    pub water: WaterTextureAssetPayload,
    pub normal_map: WaterTextureAssetPayload,
    pub caustics: WaterTextureAssetPayload,
}

impl RawTexture {
    pub fn new(name: String) -> Self {
        Self {
            name,
            width: 0,
            height: 0,
            data: Vec::new(),
            format: TextureFormat::Unknown,
            has_alpha: false,
            dds_compression: None,
        }
    }

    /// Create a solid color texture
    pub fn solid_color(name: String, width: u32, height: u32, color: [u8; 4]) -> Self {
        let pixel_count = (width * height) as usize;
        let mut data = Vec::with_capacity(pixel_count * 4);

        for _ in 0..pixel_count {
            data.extend_from_slice(&color);
        }

        Self {
            name,
            width,
            height,
            data,
            format: TextureFormat::Unknown,
            has_alpha: color[3] < 255,
            dds_compression: None,
        }
    }

    /// Create checkerboard pattern texture
    pub fn checkerboard(
        name: String,
        width: u32,
        height: u32,
        color1: [u8; 4],
        color2: [u8; 4],
    ) -> Self {
        let mut data = Vec::with_capacity((width * height * 4) as usize);
        let square_size = 8;

        for y in 0..height {
            for x in 0..width {
                let square_x = x / square_size;
                let square_y = y / square_size;
                let color = if (square_x + square_y) % 2 == 0 {
                    color1
                } else {
                    color2
                };
                data.extend_from_slice(&color);
            }
        }

        Self {
            name,
            width,
            height,
            data,
            format: TextureFormat::Unknown,
            has_alpha: color1[3] < 255 || color2[3] < 255,
            dds_compression: None,
        }
    }
}

/// GPU texture resource
pub struct GPUTexture {
    pub texture: wgpu::Texture,
    pub view: wgpu::TextureView,
    pub sampler: wgpu::Sampler,
    pub width: u32,
    pub height: u32,
}

/// Texture manager for loading and caching textures
pub struct TextureManager {
    /// Cache of loaded raw textures
    raw_cache: HashMap<String, RawTexture>,
    /// Cache of GPU textures
    gpu_cache: HashMap<String, GPUTexture>,
    /// Default white texture for fallback
    default_texture_name: String,
    /// Compact diagnostics for unresolved texture lookups.
    missing_texture_total: usize,
    missing_texture_counts: HashMap<String, usize>,
    /// Known-missing texture keys to avoid repeated archive probing.
    known_missing_textures: HashSet<String>,
}

pub fn texture_candidate_paths(requested_name: &str) -> Vec<String> {
    fn push_unique_case_insensitive(
        items: &mut Vec<String>,
        seen: &mut HashSet<String>,
        value: String,
    ) {
        if seen.insert(value.to_ascii_lowercase()) {
            items.push(value);
        }
    }

    let normalized = requested_name
        .trim()
        .replace('\\', "/")
        .trim_start_matches("./")
        .to_string();
    if normalized.is_empty() {
        return Vec::new();
    }

    let file_part = match normalized.rsplit_once('/') {
        Some((_, file)) if !file.is_empty() => file,
        _ => normalized.as_str(),
    };

    let ext_hint = match file_part.rsplit_once('.') {
        Some((stem, ext)) if !stem.is_empty() => Some(ext.to_ascii_lowercase()),
        _ => None,
    };

    fn swap_texture_extension(path: &str) -> Option<String> {
        let (stem, ext) = path.rsplit_once('.')?;
        if stem.is_empty() {
            return None;
        }
        if ext.eq_ignore_ascii_case("tga") {
            return Some(format!("{stem}.dds"));
        }
        if ext.eq_ignore_ascii_case("dds") {
            return Some(format!("{stem}.tga"));
        }
        None
    }

    let mut candidates = Vec::new();
    let mut seen_candidates = HashSet::new();

    // C++ parity: always probe the authored name first.
    push_unique_case_insensitive(&mut candidates, &mut seen_candidates, normalized.clone());
    if let Some(swapped) = swap_texture_extension(&normalized) {
        push_unique_case_insensitive(&mut candidates, &mut seen_candidates, swapped);
    }

    let is_known_texture_ext = matches!(
        ext_hint.as_deref(),
        Some("dds") | Some("tga") | Some("bmp") | Some("jpg") | Some("jpeg") | Some("png")
    );
    if is_known_texture_ext {
        for prefix in ["Data/English/Art/Textures/", "Art/Textures/"] {
            let candidate = format!("{prefix}{normalized}");
            push_unique_case_insensitive(&mut candidates, &mut seen_candidates, candidate);
            if let Some(swapped) = swap_texture_extension(&format!("{prefix}{normalized}")) {
                push_unique_case_insensitive(&mut candidates, &mut seen_candidates, swapped);
            }
        }
    }

    candidates
}

impl Default for TextureManager {
    fn default() -> Self {
        Self::new()
    }
}

impl TextureManager {
    fn normalize_texture_lookup_key(texture_name: &str) -> String {
        texture_name.trim().to_ascii_lowercase()
    }

    fn resolved_cache_key_for_lookup(&self, texture_name: &str) -> String {
        Self::normalize_texture_lookup_key(texture_name)
    }

    /// Create new texture manager
    pub fn new() -> Self {
        Self {
            raw_cache: HashMap::new(),
            gpu_cache: HashMap::new(),
            default_texture_name: "default_white".to_string(),
            missing_texture_total: 0,
            missing_texture_counts: HashMap::new(),
            known_missing_textures: HashSet::new(),
        }
    }

    /// Initialize with default textures
    pub fn init(&mut self, device: &wgpu::Device, queue: &wgpu::Queue) -> Result<()> {
        info!("Initializing texture manager");

        // Create default MAGENTA texture for missing textures - MATCHES C++ MissingTexture class
        // C++ code uses 0x7FFF00FF which represents ARGB with:
        // - 0x7F = 50% alpha/opacity in Westwood format
        // - FF = Red channel = 255
        // - 00 = Green channel = 0
        // - FF = Blue channel = 255
        // Result: Bright magenta/purple (0xFF, 0x00, 0xFF) at full opacity
        // This is INTENTIONALLY bright and obvious for visual debugging of missing textures
        // Ref: /GeneralsMD/Code/Libraries/Source/WWVegas/WW3D2/missingtexture.cpp lines 60-98
        let missing_texture = RawTexture::solid_color(
            self.default_texture_name.clone(),
            64,
            64,
            [255, 0, 255, 255], // MAGENTA (0xFF, 0x00, 0xFF) - Matches C++ MissingTexture
        );
        let gpu_texture = self.create_gpu_texture(device, queue, &missing_texture)?;

        self.raw_cache
            .insert(self.default_texture_name.clone(), missing_texture);
        self.gpu_cache
            .insert(self.default_texture_name.clone(), gpu_texture);

        // Create other useful default textures
        let green_texture =
            RawTexture::solid_color("default_green".to_string(), 64, 64, [100, 150, 50, 255]);
        let gpu_green = self.create_gpu_texture(device, queue, &green_texture)?;
        self.raw_cache
            .insert("default_green".to_string(), green_texture);
        self.gpu_cache
            .insert("default_green".to_string(), gpu_green);

        let checkerboard = RawTexture::checkerboard(
            "default_checkerboard".to_string(),
            64,
            64,
            [200, 200, 200, 255],
            [100, 100, 100, 255],
        );
        let gpu_check = self.create_gpu_texture(device, queue, &checkerboard)?;
        self.raw_cache
            .insert("default_checkerboard".to_string(), checkerboard);
        self.gpu_cache
            .insert("default_checkerboard".to_string(), gpu_check);

        info!(
            "Texture manager initialized with {} default textures",
            self.gpu_cache.len()
        );
        Ok(())
    }

    /// Load texture from BIG archive - C++ WW3DAssetManager::Get_Texture equivalent
    pub async fn load_texture(
        &mut self,
        archive_system: &mut ArchiveFileSystem,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        texture_name: &str,
    ) -> Result<&GPUTexture> {
        let texture_key = self
            .ensure_raw_texture_cached(archive_system, texture_name)
            .await?;

        if texture_key == self.default_texture_name {
            return Ok(self.get_default_texture());
        }

        if self.known_missing_textures.contains(&texture_key) {
            return Ok(self.get_default_texture());
        }

        // Return cached texture if available
        if self.gpu_cache.contains_key(&texture_key) {
            return Ok(self
                .gpu_cache
                .get(&texture_key)
                .expect("gpu_cache key existed but value disappeared"));
        }

        let raw_texture = self
            .raw_cache
            .get(&texture_key)
            .ok_or_else(|| anyhow!("Raw texture '{}' not cached after load", texture_key))?;
        let gpu_texture = self.create_gpu_texture(device, queue, raw_texture)?;
        self.gpu_cache.insert(texture_key.clone(), gpu_texture);

        // Safe unwrap since we just inserted it
        Ok(self
            .gpu_cache
            .get(&texture_key)
            .expect("Just inserted texture should be in cache"))
    }

    /// Prime and cache only raw texture payload (no GPU upload).
    pub async fn prime_raw_texture(
        &mut self,
        archive_system: &mut ArchiveFileSystem,
        texture_name: &str,
    ) -> Result<()> {
        let _ = self
            .ensure_raw_texture_cached(archive_system, texture_name)
            .await?;
        Ok(())
    }

    async fn ensure_raw_texture_cached(
        &mut self,
        archive_system: &mut ArchiveFileSystem,
        texture_name: &str,
    ) -> Result<String> {
        let requested_name = texture_name.trim();
        if requested_name.is_empty() || requested_name.eq_ignore_ascii_case("none") {
            return Ok(self.default_texture_name.clone());
        }

        let texture_key = Self::normalize_texture_lookup_key(requested_name);
        if self.raw_cache.contains_key(&texture_key)
            || self.known_missing_textures.contains(&texture_key)
        {
            return Ok(texture_key);
        }

        debug!("Loading raw texture from archive: {}", requested_name);
        let mut last_error = None;
        for candidate in Self::build_texture_candidates(requested_name) {
            let texture_data = match archive_system.open_file(&candidate).await {
                Ok(data) => data,
                Err(err) => {
                    last_error = Some((candidate, err));
                    continue;
                }
            };

            let format = TextureFormat::from_filename(&candidate);
            let parse_result = match format {
                TextureFormat::TGA => self.parse_tga(&texture_data, requested_name.to_string()),
                TextureFormat::DDS => self.parse_dds(&texture_data, requested_name.to_string()),
                TextureFormat::BMP => self.parse_bmp(&texture_data, requested_name.to_string()),
                _ => {
                    warn!(
                        "Unsupported texture format for '{}', using fallback",
                        requested_name
                    );
                    self.cache_missing_fallback(&texture_key, requested_name);
                    return Ok(texture_key);
                }
            };

            match parse_result {
                Ok(raw_texture) => {
                    self.raw_cache.insert(texture_key.clone(), raw_texture);
                    self.known_missing_textures.remove(&texture_key);
                    return Ok(texture_key);
                }
                Err(err) => {
                    warn!(
                        "Texture parse failed for '{}' from '{}': {}",
                        requested_name, candidate, err
                    );
                    last_error = Some((candidate, anyhow!("{}", err)));
                }
            }
        }

        if let Some((candidate, err)) = last_error {
            warn!(
                "Texture '{}' could not be loaded from '{}': {}",
                requested_name, candidate, err
            );
        }
        self.cache_missing_fallback(&texture_key, requested_name);
        Ok(texture_key)
    }

    fn cache_missing_fallback(&mut self, texture_key: &str, requested_name: &str) {
        let already_known = self.known_missing_textures.contains(texture_key);
        self.known_missing_textures.insert(texture_key.to_string());
        if !already_known {
            self.record_missing_texture(requested_name);
        }
    }

    fn record_missing_texture(&mut self, requested_name: &str) {
        self.missing_texture_total += 1;
        let key = requested_name.to_ascii_lowercase();
        let entry = self.missing_texture_counts.entry(key.clone()).or_insert(0);
        *entry += 1;

        // Production missing-asset diagnostic (no silent gameplay assumption).
        crate::release_candidate::note_missing_texture_fallback(requested_name);

        // Keep logs compact: emit detailed misses only for first occurrences.
        if self.missing_texture_total <= 16 {
            warn!(
                "Missing texture fallback: '{}' (total_misses={}, unique={})",
                requested_name,
                self.missing_texture_total,
                self.missing_texture_counts.len()
            );
            return;
        }

        // Periodic compact summary for long runs.
        if self.missing_texture_total.is_multiple_of(64) {
            let mut top: Vec<(&String, &usize)> = self.missing_texture_counts.iter().collect();
            top.sort_by(|a, b| b.1.cmp(a.1).then_with(|| a.0.cmp(b.0)));
            let summary: Vec<String> = top
                .into_iter()
                .take(8)
                .map(|(name, count)| format!("{name}({count})"))
                .collect();
            warn!(
                "Missing texture summary: total_misses={}, unique={}, top={:?}",
                self.missing_texture_total,
                self.missing_texture_counts.len(),
                summary
            );
        }
    }

    fn build_texture_candidates(requested_name: &str) -> Vec<String> {
        texture_candidate_paths(requested_name)
    }

    /// C++ parity: texture lookup is resolved per request using exact candidate paths.
    /// This method remains as a compatibility hook and does not prebuild an archive index.
    pub fn warmup_texture_path_index(&mut self, archive_system: &ArchiveFileSystem) -> usize {
        let _ = archive_system;
        0
    }

    /// Get texture by name (loads if not cached), returns default on error
    pub async fn get_texture_or_default(
        &mut self,
        archive_system: &mut ArchiveFileSystem,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        texture_name: &str,
    ) -> &GPUTexture {
        let texture_key = self.resolved_cache_key_for_lookup(texture_name);
        let default_key = self.default_texture_name.clone();

        // Return cached texture if available
        if self.gpu_cache.contains_key(&texture_key) {
            return self
                .gpu_cache
                .get(&texture_key)
                .expect("gpu_cache key existed but value disappeared");
        }

        if self.is_known_missing_texture(texture_name) {
            return self
                .gpu_cache
                .get(&default_key)
                .unwrap_or_else(|| self.get_default_texture());
        }

        // Try to load new texture
        if let Err(e) = self
            .load_texture(archive_system, device, queue, texture_name)
            .await
        {
            error!("Failed to load texture {}: {}", texture_name, e);
        }

        let resolved_key = self.resolved_cache_key_for_lookup(texture_name);
        if let Some(texture) = self.gpu_cache.get(&resolved_key) {
            return texture;
        }

        if self.is_known_missing_texture(texture_name) {
            return self.gpu_cache.get(&default_key).unwrap_or_else(|| {
                panic!(
                    "Known-missing texture fallback unavailable: {}",
                    texture_name
                )
            });
        }

        if let Some(default_tex) = self.gpu_cache.get(&default_key) {
            return default_tex;
        }

        warn!("Default texture not available, returning first available texture");
        self.gpu_cache
            .values()
            .next()
            .unwrap_or_else(|| panic!("No textures available at all!"))
    }

    /// Get default MAGENTA texture (for missing textures - matches C++ behavior)
    /// Returns the magenta fallback texture created during initialization
    /// This is intentionally bright and obvious for visual debugging
    pub fn get_default_texture(&self) -> &GPUTexture {
        self.gpu_cache
            .get(&self.default_texture_name)
            .expect("Default texture not initialized")
    }

    /// Get a colored default texture (for indicating different states)
    pub fn get_colored_default_texture(&self, color_name: &str) -> &GPUTexture {
        // Try to get the requested colored texture
        if let Some(texture) = self.gpu_cache.get(color_name) {
            texture
        } else {
            // Fall back to white default if color not found
            self.get_default_texture()
        }
    }

    /// Get a cached texture if it exists
    pub fn get_cached_texture(&self, texture_name: &str) -> Option<&GPUTexture> {
        let texture_key = self.resolved_cache_key_for_lookup(texture_name);
        self.gpu_cache.get(&texture_key)
    }

    /// Get cached raw texture data if available.
    pub fn get_raw_texture(&self, texture_name: &str) -> Option<&RawTexture> {
        let texture_key = self.resolved_cache_key_for_lookup(texture_name);
        self.raw_cache.get(&texture_key)
    }

    pub async fn load_water_texture_assets(
        &mut self,
        archive_system: &mut ArchiveFileSystem,
    ) -> Result<WaterTextureAssetSet> {
        let water_name = self
            .load_first_available_raw_texture(archive_system, ["TWWater01.tga"])
            .await?;
        let normal_map_name = self
            .load_first_available_raw_texture(archive_system, Self::water_normal_map_candidates())
            .await?;
        let caustics_name = self
            .load_first_available_raw_texture(archive_system, Self::water_caustics_candidates())
            .await?;

        Ok(WaterTextureAssetSet {
            water: self.clone_water_texture_asset_payload(&water_name)?,
            normal_map: self.clone_water_texture_asset_payload(&normal_map_name)?,
            caustics: self.clone_water_texture_asset_payload(&caustics_name)?,
        })
    }

    pub fn is_known_missing_texture(&self, texture_name: &str) -> bool {
        let texture_key = self.resolved_cache_key_for_lookup(texture_name);
        self.known_missing_textures.contains(&texture_key)
    }

    /// Total times a missing texture fallback was recorded (production diagnostic).
    pub fn missing_texture_total(&self) -> usize {
        self.missing_texture_total
    }

    /// Record a missing texture via the same production fallback path (for verification).
    pub fn record_missing_texture_for_verification(&mut self, requested_name: &str) {
        let key = requested_name.to_ascii_lowercase();
        self.cache_missing_fallback(&key, requested_name);
    }

    fn clone_water_texture_asset_payload(
        &self,
        texture_name: &str,
    ) -> Result<WaterTextureAssetPayload> {
        let raw = self
            .get_raw_texture(texture_name)
            .ok_or_else(|| anyhow!("Water texture '{}' was not cached", texture_name))?;

        Ok(WaterTextureAssetPayload {
            name: texture_name.to_string(),
            width: raw.width,
            height: raw.height,
            rgba: raw.data.clone(),
        })
    }

    async fn load_first_available_raw_texture<I, S>(
        &mut self,
        archive_system: &mut ArchiveFileSystem,
        candidates: I,
    ) -> Result<String>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        for candidate in candidates {
            let candidate = candidate.as_ref();
            self.prime_raw_texture(archive_system, candidate).await?;
            if self.get_raw_texture(candidate).is_some()
                && !self.is_known_missing_texture(candidate)
            {
                return Ok(candidate.to_string());
            }
        }

        Err(anyhow!(
            "Failed to load required water texture assets from BIG archives"
        ))
    }

    fn water_normal_map_candidates() -> Vec<String> {
        let mut candidates = vec!["Noise0000.tga".to_string()];
        for i in 0..32 {
            candidates.push(format!("caustS{:02}.tga", i));
        }
        for i in 0..32 {
            candidates.push(format!("caust{:02}.tga", i));
        }
        candidates
    }

    fn water_caustics_candidates() -> Vec<String> {
        let mut candidates = Vec::with_capacity(64);
        for i in 0..32 {
            candidates.push(format!("caust{:02}.tga", i));
        }
        for i in 0..32 {
            candidates.push(format!("caustS{:02}.tga", i));
        }
        candidates
    }

    /// Create GPU texture from raw data
    fn create_gpu_texture(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        raw: &RawTexture,
    ) -> Result<GPUTexture> {
        if let Some(compression) = raw.dds_compression {
            if device
                .features()
                .contains(wgpu::Features::TEXTURE_COMPRESSION_BC)
            {
                let block_size = compression.block_size_bytes();
                let blocks_x = raw.width.div_ceil(4);
                let blocks_y = raw.height.div_ceil(4);
                let expected_size = compression.expected_payload_size(raw.width, raw.height);

                if raw.data.len() >= expected_size {
                    let texture_size = wgpu::Extent3d {
                        width: raw.width,
                        height: raw.height,
                        depth_or_array_layers: 1,
                    };

                    let texture = device.create_texture(&wgpu::TextureDescriptor {
                        label: Some(&raw.name),
                        size: texture_size,
                        mip_level_count: 1,
                        sample_count: 1,
                        dimension: wgpu::TextureDimension::D2,
                        format: compression.to_wgpu_format(),
                        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
                        view_formats: &[],
                    });

                    queue.write_texture(
                        wgpu::TexelCopyTextureInfo {
                            aspect: wgpu::TextureAspect::All,
                            texture: &texture,
                            mip_level: 0,
                            origin: wgpu::Origin3d::ZERO,
                        },
                        &raw.data[..expected_size],
                        wgpu::TexelCopyBufferLayout {
                            offset: 0,
                            bytes_per_row: Some(block_size * blocks_x),
                            rows_per_image: Some(blocks_y),
                        },
                        texture_size,
                    );

                    let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
                    let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
                        address_mode_u: wgpu::AddressMode::Repeat,
                        address_mode_v: wgpu::AddressMode::Repeat,
                        address_mode_w: wgpu::AddressMode::Repeat,
                        mag_filter: wgpu::FilterMode::Linear,
                        min_filter: wgpu::FilterMode::Nearest,
                        mipmap_filter: wgpu::FilterMode::Nearest,
                        ..Default::default()
                    });

                    return Ok(GPUTexture {
                        texture,
                        view,
                        sampler,
                        width: raw.width,
                        height: raw.height,
                    });
                }

                warn!(
                    "DDS '{}' compressed payload size mismatch (expected {}, got {}), falling back to CPU decode",
                    raw.name,
                    expected_size,
                    raw.data.len()
                );
            } else {
                debug!(
                    "DDS '{}' BC formats unsupported by adapter, falling back to CPU decode",
                    raw.name
                );
            }

            let decode_result = match compression {
                DdsCompression::Dxt1 => decode_dxt1(&raw.data, raw.width, raw.height)
                    .map_err(|e| anyhow!("DXT1 decode: {}", e)),
                DdsCompression::Dxt3 => decode_dxt3(&raw.data, raw.width, raw.height)
                    .map_err(|e| anyhow!("DXT3 decode: {}", e)),
                DdsCompression::Dxt5 => decode_dxt5(&raw.data, raw.width, raw.height)
                    .map_err(|e| anyhow!("DXT5 decode: {}", e)),
            };

            let rgba_data = match decode_result {
                Ok(data) => data,
                Err(e) => {
                    warn!(
                        "DDS '{}' CPU decode fallback failed; using solid fallback texture: {}",
                        raw.name, e
                    );
                    let fallback = RawTexture::solid_color(
                        raw.name.clone(),
                        raw.width.max(1),
                        raw.height.max(1),
                        [150, 100, 50, 255],
                    );
                    return self.create_gpu_texture_from_rgba(
                        device,
                        queue,
                        &fallback.name,
                        fallback.width,
                        fallback.height,
                        &fallback.data,
                    );
                }
            };

            return self.create_gpu_texture_from_rgba(
                device, queue, &raw.name, raw.width, raw.height, &rgba_data,
            );
        }

        self.create_gpu_texture_from_rgba(
            device, queue, &raw.name, raw.width, raw.height, &raw.data,
        )
    }

    fn create_gpu_texture_from_rgba(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        name: &str,
        width: u32,
        height: u32,
        rgba: &[u8],
    ) -> Result<GPUTexture> {
        let expected_size = (width as usize)
            .saturating_mul(height as usize)
            .saturating_mul(4);
        if rgba.len() < expected_size {
            return Err(anyhow!(
                "Texture '{}' RGBA payload too small: expected {}, got {}",
                name,
                expected_size,
                rgba.len()
            ));
        }

        let texture_size = wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        };

        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some(name),
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
                aspect: wgpu::TextureAspect::All,
                texture: &texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
            },
            &rgba[..expected_size],
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(4 * width),
                rows_per_image: Some(height),
            },
            texture_size,
        );

        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            address_mode_u: wgpu::AddressMode::Repeat,
            address_mode_v: wgpu::AddressMode::Repeat,
            address_mode_w: wgpu::AddressMode::Repeat,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Nearest,
            mipmap_filter: wgpu::FilterMode::Nearest,
            ..Default::default()
        });

        Ok(GPUTexture {
            texture,
            view,
            sampler,
            width,
            height,
        })
    }

    fn parse_tga(&self, data: &[u8], name: String) -> Result<RawTexture> {
        debug!("Parsing TGA texture: {}", name);

        let tga = load_tga_from_memory(data).map_err(|e| anyhow!("TGA parse error: {}", e))?;

        Ok(RawTexture {
            name,
            width: tga.width,
            height: tga.height,
            data: tga.data,
            format: TextureFormat::TGA,
            has_alpha: tga.bits_per_pixel == 32,
            dds_compression: None,
        })
    }

    fn parse_dds(&self, data: &[u8], name: String) -> Result<RawTexture> {
        debug!("Parsing DDS texture: {}", name);

        let dds = load_dds_from_memory(data).map_err(|e| anyhow!("DDS parse error: {}", e))?;

        let level0_data = dds
            .get_level_data(0)
            .ok_or_else(|| anyhow!("DDS level 0 data missing"))?;

        let has_alpha = dds
            .compression
            .is_some_and(|c| matches!(c, DdsCompression::Dxt3 | DdsCompression::Dxt5));

        Ok(RawTexture {
            name,
            width: dds.width,
            height: dds.height,
            data: level0_data.to_vec(),
            format: TextureFormat::DDS,
            has_alpha,
            dds_compression: dds.compression,
        })
    }

    /// Parse BMP texture format
    fn parse_bmp(&self, data: &[u8], name: String) -> Result<RawTexture> {
        debug!("Parsing BMP texture: {}", name);

        if data.len() < 54 {
            return Err(anyhow!("BMP file too small: {} bytes", data.len()));
        }

        // BMP header parsing based on Windows BITMAPFILEHEADER and BITMAPINFOHEADER
        if &data[0..2] != b"BM" {
            return Err(anyhow!("Invalid BMP magic number"));
        }

        let _file_size = u32::from_le_bytes([data[2], data[3], data[4], data[5]]);
        let data_offset = u32::from_le_bytes([data[10], data[11], data[12], data[13]]) as usize;
        let info_header_size = u32::from_le_bytes([data[14], data[15], data[16], data[17]]);

        if info_header_size < 40 {
            return Err(anyhow!(
                "Invalid BMP info header size: {}",
                info_header_size
            ));
        }

        let width = i32::from_le_bytes([data[18], data[19], data[20], data[21]]) as u32;
        let height_signed = i32::from_le_bytes([data[22], data[23], data[24], data[25]]);
        let height = height_signed.unsigned_abs();
        let planes = u16::from_le_bytes([data[26], data[27]]);
        let bits_per_pixel = u16::from_le_bytes([data[28], data[29]]);
        let compression = u32::from_le_bytes([data[30], data[31], data[32], data[33]]);
        let _image_size = u32::from_le_bytes([data[34], data[35], data[36], data[37]]);

        debug!(
            "BMP: {}x{}, {} bpp, compression: {}, planes: {}",
            width, height, bits_per_pixel, compression, planes
        );

        if width == 0 || height == 0 {
            return Err(anyhow!("Invalid BMP dimensions: {}x{}", width, height));
        }

        if planes != 1 {
            return Err(anyhow!("Unsupported BMP planes: {}", planes));
        }

        if compression != 0 {
            return Err(anyhow!(
                "Compressed BMP files not supported: {}",
                compression
            ));
        }

        if data_offset >= data.len() {
            return Err(anyhow!("BMP data offset beyond file size"));
        }

        let image_data = &data[data_offset..];
        let mut rgba_data = Vec::with_capacity((width * height * 4) as usize);

        // Calculate row padding (BMP rows are padded to 4-byte boundaries)
        let bytes_per_pixel = bits_per_pixel as usize / 8;
        let row_size_no_padding = width as usize * bytes_per_pixel;
        let row_padding = (4 - (row_size_no_padding % 4)) % 4;
        let row_size_with_padding = row_size_no_padding + row_padding;

        match bits_per_pixel {
            24 => {
                // 24-bit BGR format
                for row in 0..height {
                    let actual_row = if height_signed > 0 {
                        height - 1 - row
                    } else {
                        row
                    };
                    let row_start = (actual_row as usize) * row_size_with_padding;

                    if row_start + row_size_no_padding > image_data.len() {
                        return Err(anyhow!("BMP image data truncated"));
                    }

                    let row_data = &image_data[row_start..row_start + row_size_no_padding];

                    for chunk in row_data.chunks_exact(3) {
                        rgba_data.push(chunk[2]); // R
                        rgba_data.push(chunk[1]); // G
                        rgba_data.push(chunk[0]); // B
                        rgba_data.push(255); // A
                    }
                }
            }
            32 => {
                // 32-bit BGRA format
                for row in 0..height {
                    let actual_row = if height_signed > 0 {
                        height - 1 - row
                    } else {
                        row
                    };
                    let row_start = (actual_row as usize) * row_size_with_padding;

                    if row_start + row_size_no_padding > image_data.len() {
                        return Err(anyhow!("BMP image data truncated"));
                    }

                    let row_data = &image_data[row_start..row_start + row_size_no_padding];

                    for chunk in row_data.chunks_exact(4) {
                        rgba_data.push(chunk[2]); // R
                        rgba_data.push(chunk[1]); // G
                        rgba_data.push(chunk[0]); // B
                        rgba_data.push(chunk[3]); // A
                    }
                }
            }
            16 => {
                // 16-bit RGB565 format
                for row in 0..height {
                    let actual_row = if height_signed > 0 {
                        height - 1 - row
                    } else {
                        row
                    };
                    let row_start = (actual_row as usize) * row_size_with_padding;

                    if row_start + row_size_no_padding > image_data.len() {
                        return Err(anyhow!("BMP image data truncated"));
                    }

                    let row_data = &image_data[row_start..row_start + row_size_no_padding];

                    for chunk in row_data.chunks_exact(2) {
                        let pixel = u16::from_le_bytes([chunk[0], chunk[1]]);
                        rgba_data.push(((pixel & 0xF800) >> 8) as u8); // R
                        rgba_data.push(((pixel & 0x07E0) >> 3) as u8); // G
                        rgba_data.push(((pixel & 0x001F) << 3) as u8); // B
                        rgba_data.push(255); // A
                    }
                }
            }
            8 => {
                // 8-bit paletted format - need to read palette
                let palette_offset = 54; // After info header
                let palette_size = 256 * 4; // 256 colors * 4 bytes (BGRA)

                if data.len() < palette_offset + palette_size {
                    return Err(anyhow!("BMP palette data missing"));
                }

                let palette_data = &data[palette_offset..palette_offset + palette_size];

                for row in 0..height {
                    let actual_row = if height_signed > 0 {
                        height - 1 - row
                    } else {
                        row
                    };
                    let row_start = (actual_row as usize) * row_size_with_padding;

                    if row_start + row_size_no_padding > image_data.len() {
                        return Err(anyhow!("BMP image data truncated"));
                    }

                    let row_data = &image_data[row_start..row_start + row_size_no_padding];

                    for &index in row_data {
                        let palette_index = (index as usize) * 4;
                        if palette_index + 3 < palette_data.len() {
                            rgba_data.push(palette_data[palette_index + 2]); // R
                            rgba_data.push(palette_data[palette_index + 1]); // G
                            rgba_data.push(palette_data[palette_index]); // B
                            rgba_data.push(255); // A
                        } else {
                            rgba_data.extend_from_slice(&[0, 0, 0, 255]); // Black fallback
                        }
                    }
                }
            }
            _ => {
                warn!("Unsupported BMP bit depth: {}", bits_per_pixel);
                return Ok(RawTexture::solid_color(
                    name,
                    width,
                    height,
                    [50, 150, 100, 255],
                ));
            }
        }

        Ok(RawTexture {
            name,
            width,
            height,
            data: rgba_data,
            format: TextureFormat::BMP,
            has_alpha: bits_per_pixel == 32,
            dds_compression: None,
        })
    }

    /// List available textures in archives
    pub fn list_available_textures(&self, archive_system: &ArchiveFileSystem) -> Vec<String> {
        let mut textures = Vec::new();
        let all_files = archive_system.list_all_files();

        for file in all_files {
            let file_lower = file.to_lowercase();
            if file_lower.ends_with(".tga")
                || file_lower.ends_with(".dds")
                || file_lower.ends_with(".bmp")
                || file_lower.ends_with(".jpg")
                || file_lower.ends_with(".jpeg")
                || file_lower.ends_with(".png")
            {
                textures.push(file);
            }
        }

        textures.sort();
        textures
    }

    /// Load caustic animation textures for water effects
    pub async fn load_caustic_textures(
        &mut self,
        archive_system: &mut ArchiveFileSystem,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
    ) -> Result<Vec<String>> {
        info!("Loading caustic animation textures for water effects");

        let mut caustic_names = Vec::new();
        let mut all_names: Vec<String> = Vec::with_capacity(64);

        for i in 0..32 {
            all_names.push(format!("caust{:02}.tga", i));
            all_names.push(format!("caustS{:02}.tga", i));
        }

        for caustic_name in all_names {
            self.load_texture(archive_system, device, queue, &caustic_name)
                .await?;

            if !self.is_known_missing_texture(&caustic_name) {
                caustic_names.push(caustic_name);
            }
        }

        info!("Loaded {} caustic animation frames", caustic_names.len());
        Ok(caustic_names)
    }

    /// Clear texture cache
    pub fn clear_cache(&mut self) {
        self.raw_cache
            .retain(|name, _| name == &self.default_texture_name || name.starts_with("default_"));
        self.gpu_cache
            .retain(|name, _| name == &self.default_texture_name || name.starts_with("default_"));
        self.known_missing_textures.clear();
        self.missing_texture_total = 0;
        self.missing_texture_counts.clear();
    }

    /// Get cache statistics
    pub fn get_cache_stats(&self) -> (usize, usize) {
        (self.raw_cache.len(), self.gpu_cache.len())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn build_dds(width: u32, height: u32, four_cc: [u8; 4], payload: &[u8]) -> Vec<u8> {
        let mut dds = vec![0u8; 128];
        dds[0..4].copy_from_slice(b"DDS ");
        dds[4..8].copy_from_slice(&124u32.to_le_bytes()); // header size
        dds[8..12].copy_from_slice(&0x0002_1007u32.to_le_bytes()); // caps|height|width|pixelformat|linearsize
        dds[12..16].copy_from_slice(&height.to_le_bytes());
        dds[16..20].copy_from_slice(&width.to_le_bytes());
        dds[20..24].copy_from_slice(&(payload.len() as u32).to_le_bytes()); // linear size
        dds[76..80].copy_from_slice(&32u32.to_le_bytes()); // pixel format size
        dds[80..84].copy_from_slice(&0x0000_0004u32.to_le_bytes()); // DDPF_FOURCC
        dds[84..88].copy_from_slice(&four_cc);
        dds[108..112].copy_from_slice(&0x1000u32.to_le_bytes()); // DDSCAPS_TEXTURE
        dds.extend_from_slice(payload);
        dds
    }

    fn rgba_at(data: &[u8], width: u32, x: u32, y: u32) -> [u8; 4] {
        let i = ((y * width + x) * 4) as usize;
        [data[i], data[i + 1], data[i + 2], data[i + 3]]
    }

    #[test]
    fn decode_dxt1_writes_row_major_across_blocks() {
        let mut data = Vec::new();
        // Block 0 (left): solid red (index 0 for all pixels).
        data.extend_from_slice(&0xF800u16.to_le_bytes());
        data.extend_from_slice(&0x0000u16.to_le_bytes());
        data.extend_from_slice(&0u32.to_le_bytes());
        // Block 1 (right): solid green (index 0 for all pixels).
        data.extend_from_slice(&0x07E0u16.to_le_bytes());
        data.extend_from_slice(&0x0000u16.to_le_bytes());
        data.extend_from_slice(&0u32.to_le_bytes());

        let decoded = decode_dxt1(&data, 8, 4).expect("DXT1 decode should succeed");

        assert_eq!(rgba_at(&decoded, 8, 0, 0), [255, 0, 0, 255]);
        assert_eq!(rgba_at(&decoded, 8, 3, 3), [255, 0, 0, 255]);
        assert_eq!(rgba_at(&decoded, 8, 4, 0), [0, 255, 0, 255]);
        assert_eq!(rgba_at(&decoded, 8, 7, 3), [0, 255, 0, 255]);
    }

    #[test]
    fn decode_dxt3_uses_explicit_4bit_alpha() {
        let mut alpha_bits = 0u64;
        alpha_bits |= 0xF << 4; // pixel 1 -> full alpha
        alpha_bits |= 0x8 << 8; // pixel 2 -> mid alpha (8 * 17 = 136)

        let mut data = Vec::new();
        data.extend_from_slice(&alpha_bits.to_le_bytes());
        data.extend_from_slice(&0xFFFFu16.to_le_bytes()); // white
        data.extend_from_slice(&0x0000u16.to_le_bytes());
        data.extend_from_slice(&0u32.to_le_bytes());

        let decoded = decode_dxt3(&data, 4, 4).expect("DXT3 decode should succeed");

        assert_eq!(rgba_at(&decoded, 4, 0, 0), [255, 255, 255, 0]);
        assert_eq!(rgba_at(&decoded, 4, 1, 0), [255, 255, 255, 255]);
        assert_eq!(rgba_at(&decoded, 4, 2, 0), [255, 255, 255, 136]);
    }

    #[test]
    fn texture_candidate_paths_have_cpp_roots_only() {
        let candidates = texture_candidate_paths("  PTPalm02a.tga ");

        assert_eq!(candidates[0], "PTPalm02a.tga");
        assert_eq!(candidates[1], "PTPalm02a.dds");
        assert_eq!(candidates[2], "Data/English/Art/Textures/PTPalm02a.tga");
        assert_eq!(candidates[3], "Data/English/Art/Textures/PTPalm02a.dds");
        assert_eq!(candidates[4], "Art/Textures/PTPalm02a.tga");
        assert_eq!(candidates[5], "Art/Textures/PTPalm02a.dds");
    }

    #[test]
    fn texture_candidate_paths_ignore_terrain_w3d_aliases() {
        let candidates = texture_candidate_paths("PTPalm02a.tga");

        assert!(candidates.iter().all(|path| !path.contains("Terrain")));
        assert!(candidates.iter().all(|path| !path.contains("W3D/")));
    }

    #[test]
    fn texture_candidate_paths_normalize_backslashes() {
        let candidates = TextureManager::build_texture_candidates("Art\\W3D\\PTXBIRCH05.tga");

        assert_eq!(candidates[0], "Art/W3D/PTXBIRCH05.tga");
        assert_eq!(candidates[1], "Art/W3D/PTXBIRCH05.dds");
        assert_eq!(
            candidates[2],
            "Data/English/Art/Textures/Art/W3D/PTXBIRCH05.tga"
        );
        assert_eq!(
            candidates[3],
            "Data/English/Art/Textures/Art/W3D/PTXBIRCH05.dds"
        );
        assert_eq!(candidates[4], "Art/Textures/Art/W3D/PTXBIRCH05.tga");
        assert_eq!(candidates[5], "Art/Textures/Art/W3D/PTXBIRCH05.dds");
    }

    #[test]
    fn resolved_cache_key_preserves_request_spelling() {
        let manager = TextureManager::new();

        assert_eq!(
            manager.resolved_cache_key_for_lookup("  Art/Textures/PTPalm02a.tga  "),
            "art/textures/ptpalm02a.tga"
        );
        assert_eq!(
            manager.resolved_cache_key_for_lookup("PTPalm02a.tga"),
            "ptpalm02a.tga"
        );
    }

    #[test]
    fn decode_dxt5_uses_interpolated_alpha_table() {
        let alpha_indices = [0u64, 1, 2, 3, 4, 5, 6, 7];
        let mut alpha_index_bits = 0u64;
        for (i, idx) in alpha_indices.iter().enumerate() {
            alpha_index_bits |= *idx << (3 * i);
        }

        let mut data = vec![0u8; 16];
        data[0] = 0;
        data[1] = 255;
        for i in 0..6usize {
            data[2 + i] = ((alpha_index_bits >> (8 * i)) & 0xFF) as u8;
        }
        data[8..10].copy_from_slice(&0xFFFFu16.to_le_bytes()); // white
        data[10..12].copy_from_slice(&0x0000u16.to_le_bytes());
        data[12..16].copy_from_slice(&0u32.to_le_bytes());

        let decoded = decode_dxt5(&data, 4, 4).expect("DXT5 decode should succeed");

        assert_eq!(rgba_at(&decoded, 4, 0, 0)[3], 0);
        assert_eq!(rgba_at(&decoded, 4, 1, 0)[3], 255);
        assert_eq!(rgba_at(&decoded, 4, 2, 0)[3], 51);
        assert_eq!(rgba_at(&decoded, 4, 3, 0)[3], 102);
        assert_eq!(rgba_at(&decoded, 4, 0, 1)[3], 153);
        assert_eq!(rgba_at(&decoded, 4, 1, 1)[3], 204);
        assert_eq!(rgba_at(&decoded, 4, 2, 1)[3], 0);
        assert_eq!(rgba_at(&decoded, 4, 3, 1)[3], 255);
    }

    #[test]
    fn dds_compression_maps_to_bc_formats() {
        assert_eq!(
            DdsCompression::Dxt1.to_wgpu_format(),
            wgpu::TextureFormat::Bc1RgbaUnormSrgb
        );
        assert_eq!(
            DdsCompression::Dxt3.to_wgpu_format(),
            wgpu::TextureFormat::Bc2RgbaUnormSrgb
        );
        assert_eq!(
            DdsCompression::Dxt5.to_wgpu_format(),
            wgpu::TextureFormat::Bc3RgbaUnormSrgb
        );
    }

    #[test]
    fn parse_dds_dxt1_keeps_compressed_payload_for_gpu_upload() {
        let manager = TextureManager::new();
        let payload = vec![0x00, 0xF8, 0x00, 0x00, 0, 0, 0, 0];
        let dds = build_dds(4, 4, *b"DXT1", &payload);

        let raw = manager
            .parse_dds(&dds, "test_dxt1.dds".to_string())
            .expect("DDS parse should succeed");

        assert_eq!(raw.width, 4);
        assert_eq!(raw.height, 4);
        assert!(matches!(raw.format, TextureFormat::DDS));
        assert_eq!(raw.dds_compression, Some(DdsCompression::Dxt1));
        assert_eq!(raw.data, payload);
    }
}
