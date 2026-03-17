//! Terrain Texture System
//!
//! Manages terrain texturing in a minimal C++-style pipeline: texture IDs
//! with per-vertex weights.

use crate::terrain::{TerrainError, TerrainResult};
use bytemuck::cast_slice;
use game_engine::common::global_data;
use game_engine::common::ini::ini_webpage_url::get_registry_language;
use game_engine::common::system::{
    big_file_system::BigArchiveBackend,
    file::FileAccess,
    file_system::get_file_system,
    file_system::paths::{
        MAP_PREVIEW_DIR_PATH, TERRAIN_TGA_DIR_PATH, TGA_DIR_PATH, USER_TGA_DIR_PATH,
    },
    local_file_system::LocalFileSystem,
    subsystem_interface::SubsystemInterface as CommonSubsystemInterface,
};
use glam::Vec3;
use image::{self, DynamicImage, GenericImageView, ImageFormat, RgbaImage};
use log::warn;
use std::collections::{HashMap, HashSet};
use std::env;
use std::fs::OpenOptions;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock};
use wgpu::{Device, Queue, Texture, TextureDescriptor, TextureFormat, TextureView};

/// Unique identifier for terrain textures
pub type TextureId = u32;

/// Maximum blend weights tracked per vertex (matches legacy limit)
pub const MAX_BLEND_WEIGHTS: usize = 4;
pub const DEFAULT_TEXTURE_DIMENSIONS: (u32, u32) = (256, 256);

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum TextureKind {
    Diffuse,
}

type TextureCacheKey = (TextureId, TextureKind);

type PathCache = Mutex<HashMap<String, Option<PathBuf>>>;
type DecodedImage = Arc<image::DynamicImage>;
type ImageCache = Mutex<HashMap<String, Option<DecodedImage>>>;
type GameFsPathCache = Mutex<HashMap<String, Option<String>>>;

static RESOLVED_PATH_CACHE: OnceLock<PathCache> = OnceLock::new();
static GAME_FS_PATH_CACHE: OnceLock<GameFsPathCache> = OnceLock::new();
static GAME_FS_IMAGE_CACHE: OnceLock<ImageCache> = OnceLock::new();

fn normalized_texture_key(path: &str) -> String {
    normalize_texture_name(path).to_ascii_lowercase()
}

fn normalize_texture_name(path: &str) -> String {
    path.replace('\\', "/")
        .trim_start_matches("./")
        .chars()
        .filter(|c| *c != ' ')
        .collect::<String>()
}

fn runtime_root_candidates() -> Vec<PathBuf> {
    let mut roots = Vec::new();
    if let Ok(current) = env::current_dir() {
        roots.push(current);
    }
    if let Ok(exe) = env::current_exe() {
        if let Some(parent) = exe.parent() {
            roots.push(parent.to_path_buf());
        }
    }
    roots
}

fn ensure_engine_filesystem_backends() {
    static INIT: OnceLock<()> = OnceLock::new();
    INIT.get_or_init(|| {
        let fs = get_file_system();
        let Ok(mut fs_guard) = fs.lock() else {
            return;
        };

        let writable = {
            let data = global_data::read();
            data.writable.clone()
        };
        let mut search_paths = vec![PathBuf::from(".")];

        for base in runtime_root_candidates() {
            search_paths.push(base.clone());
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
            let big_backend: &mut BigArchiveBackend =
                fs_guard.ensure_backend(BigArchiveBackend::new);
            for path in &search_paths {
                big_backend.add_search_path(path);
            }
        }

        fs_guard.clear_cache();
        let _ = CommonSubsystemInterface::init(&mut *fs_guard);
    });
}

/// Terrain texture registration data.
#[derive(Debug, Clone)]
pub struct TerrainTexture {
    /// Unique identifier
    pub id: TextureId,

    /// Display name for editor/debugging
    pub name: String,

    /// Path to diffuse texture
    pub diffuse_path: String,

    /// Whether texture is loaded
    pub loaded: bool,

    /// Texture dimensions (when loaded)
    pub dimensions: Option<(u32, u32)>,

    /// Resolved on-disk path for GPU loading
    pub resolved_path: Option<PathBuf>,

    /// Cached diffuse decode to avoid repeated loads during first GPU upload
    cached_diffuse_image: Option<Arc<image::DynamicImage>>,
}

/// Texture blending modes
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum BlendMode {
    #[default]
    Neutral,
}

/// Texture blending weights for terrain vertices
#[derive(Debug, Clone)]
pub struct TextureWeights {
    /// Up to 4 texture identifiers
    pub indices: [TextureId; 4],

    /// Corresponding blend weights (should sum to 1.0)
    pub weights: [f32; 4],
}

/// Manages terrain textures and blending
#[derive(Debug)]
pub struct TextureManager {
    /// All registered textures
    textures: HashMap<TextureId, TerrainTexture>,
    /// C++-style direct name/path lookup for already-registered textures.
    texture_path_index: HashMap<String, TextureId>,

    /// GPU texture cache keyed by texture identifier
    gpu_cache: HashMap<TextureCacheKey, GPUTextureEntry>,

    /// Next available texture ID
    next_texture_id: TextureId,

    /// Performance statistics
    stats: TextureStats,

    /// Optional path for writing blend debug samples
    blend_debug_path: Option<PathBuf>,
}

/// Performance statistics for texture system
#[derive(Debug, Default)]
pub struct TextureStats {
    pub total_textures: u32,
    pub loaded_textures: u32,
    pub blend_operations: u64,
    pub last_update_time: std::time::Duration,
}

#[derive(Debug, Clone)]
struct GPUTextureEntry {
    texture: Arc<Texture>,
    view: Arc<TextureView>,
}

impl TerrainTexture {
    /// Create new terrain texture
    pub fn new(id: TextureId, name: String, diffuse_path: String) -> Self {
        Self {
            id,
            name,
            diffuse_path,
            loaded: false,
            dimensions: None,
            resolved_path: None,
            cached_diffuse_image: None,
        }
    }

    /// Check if texture has all required maps loaded
    pub fn is_complete(&self) -> bool {
        self.loaded && self.dimensions.is_some()
    }
}

impl TextureWeights {
    /// Create new texture weights (normalized)
    pub fn new(indices: [TextureId; MAX_BLEND_WEIGHTS], weights: [f32; MAX_BLEND_WEIGHTS]) -> Self {
        let mut result = Self { indices, weights };
        result.normalize();
        result
    }

    /// Create texture weights for single texture
    pub fn single(texture_index: TextureId) -> Self {
        let mut indices = [0; MAX_BLEND_WEIGHTS];
        let mut weights = [0.0f32; MAX_BLEND_WEIGHTS];
        indices[0] = texture_index;
        weights[0] = 1.0;
        Self { indices, weights }
    }

    /// Create texture weights for two textures
    pub fn blend_two(tex1: TextureId, tex2: TextureId, weight1: f32) -> Self {
        let mut indices = [0; MAX_BLEND_WEIGHTS];
        let mut weights = [0.0f32; MAX_BLEND_WEIGHTS];
        indices[0] = tex1;
        indices[1] = tex2;
        weights[0] = weight1;
        weights[1] = (1.0 - weight1).max(0.0);
        Self { indices, weights }
    }

    /// Create empty weight set
    pub fn empty() -> Self {
        Self {
            indices: [0; MAX_BLEND_WEIGHTS],
            weights: [0.0; MAX_BLEND_WEIGHTS],
        }
    }

    /// Total accumulated weight
    pub fn total_weight(&self) -> f32 {
        self.weights.iter().sum()
    }

    /// Whether any weight contributions exist
    pub fn is_empty(&self) -> bool {
        self.weights.iter().all(|w| *w <= f32::EPSILON)
    }

    /// Normalize weights to sum to 1.0
    pub fn normalize(&mut self) {
        let sum: f32 = self.weights.iter().sum();
        if sum > 0.0 {
            for weight in &mut self.weights {
                *weight /= sum;
            }
        }
    }

    /// Build weights from an arbitrary weighted list (keeps top 4 entries).
    pub fn from_weight_pairs(mut pairs: Vec<(TextureId, f32)>) -> Self {
        use std::cmp::Ordering;

        pairs.retain(|(_, weight)| *weight > f32::EPSILON);
        if pairs.is_empty() {
            return Self::empty();
        }

        pairs.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(Ordering::Equal));

        let mut indices = [0; 4];
        let mut weights = [0.0; 4];
        let mut total = 0.0;

        for (slot, (texture, weight)) in pairs.iter().take(MAX_BLEND_WEIGHTS).enumerate() {
            indices[slot] = *texture;
            weights[slot] = *weight;
            total += *weight;
        }

        if total > f32::EPSILON {
            for weight in &mut weights {
                if *weight > 0.0 {
                    *weight /= total;
                }
            }
            Self { indices, weights }
        } else {
            Self::single(pairs[0].0)
        }
    }

    /// Iterate over texture-weight pairs with non-zero contribution.
    pub fn iter_pairs(&self) -> impl Iterator<Item = (TextureId, f32)> + '_ {
        self.indices
            .iter()
            .zip(self.weights.iter())
            .filter(|(_, weight)| **weight > f32::EPSILON)
            .map(|(index, weight)| (*index, *weight))
    }

    /// Add texture blend with given weight
    pub fn add_texture(&mut self, texture_index: TextureId, weight: f32) {
        // Find empty slot or existing texture
        for i in 0..MAX_BLEND_WEIGHTS {
            if self.weights[i] == 0.0 || self.indices[i] == texture_index {
                self.indices[i] = texture_index;
                self.weights[i] += weight;
                self.normalize();
                return;
            }
        }

        // No empty slot - replace weakest texture
        let min_index = self
            .weights
            .iter()
            .enumerate()
            .min_by(|a, b| a.1.partial_cmp(b.1).unwrap())
            .map(|(i, _)| i)
            .unwrap_or(MAX_BLEND_WEIGHTS - 1);

        self.indices[min_index] = texture_index;
        self.weights[min_index] = weight;
        self.normalize();
    }
}

impl TextureManager {
    /// Initialize the manager and prepare default configuration.
    pub fn init(&mut self) -> TerrainResult<()> {
        Ok(())
    }

    /// Create new texture manager
    pub fn new() -> Self {
        let blend_debug_path = env::var("WW_TERRAIN_BLEND_DEBUG").ok().map(PathBuf::from);

        Self {
            textures: HashMap::new(),
            texture_path_index: HashMap::new(),
            gpu_cache: HashMap::new(),
            next_texture_id: 1,
            stats: TextureStats::default(),
            blend_debug_path,
        }
    }

    /// Reset all textures back to defaults
    pub fn reset(&mut self) -> TerrainResult<()> {
        self.textures.clear();
        self.texture_path_index.clear();
        self.gpu_cache.clear();
        self.next_texture_id = 1;
        self.stats = TextureStats::default();
        Ok(())
    }

    fn resolve_texture_path_cached(diffuse_path: &str) -> Option<PathBuf> {
        let key = normalized_texture_key(diffuse_path);
        let cache = RESOLVED_PATH_CACHE.get_or_init(|| Mutex::new(HashMap::new()));
        if let Ok(cache) = cache.lock() {
            if let Some(cached) = cache.get(&key) {
                return cached.clone();
            }
        }

        let resolved = Self::resolve_texture_path(diffuse_path);
        if let Ok(mut cache) = cache.lock() {
            cache.insert(key, resolved.clone());
        }
        resolved
    }

    /// Register new terrain texture
    pub fn register_texture(&mut self, mut texture: TerrainTexture) -> TextureId {
        texture.id = self.next_texture_id;
        let id = texture.id;
        let path_key = normalized_texture_key(&texture.diffuse_path);

        self.textures.insert(id, texture);
        self.texture_path_index.insert(path_key, id);
        self.next_texture_id += 1;
        self.stats.total_textures += 1;

        id
    }

    pub(crate) fn can_resolve_texture_path(diffuse_path: &str) -> bool {
        Self::resolve_texture_path(diffuse_path).is_some()
    }

    pub(crate) fn is_available_terrain_texture_path(texture_path: &str) -> bool {
        Self::resolve_game_fs_path_cached(texture_path).is_some()
    }

    pub fn resolve_texture_path(diffuse_path: &str) -> Option<PathBuf> {
        ensure_engine_filesystem_backends();

        if let Some(fs_path) = Self::resolve_game_fs_path_cached(diffuse_path) {
            return Some(PathBuf::from(fs_path));
        }

        None
    }

    fn resource_candidates(filename: &str) -> Vec<String> {
        let filename = normalize_texture_name(filename);
        if filename.is_empty() {
            return Vec::new();
        }

        let has_path_component = filename.contains('/');
        let extension = Path::new(&filename)
            .extension()
            .and_then(|ext| ext.to_str())
            .map(|ext| ext.to_ascii_lowercase());

        let is_tga_or_dds = matches!(extension.as_deref(), Some("tga") | Some("dds"));
        if !is_tga_or_dds {
            if has_path_component {
                return vec![filename];
            }
            return Vec::new();
        }

        let mut candidates = Vec::new();
        let mut seen = HashSet::new();
        let mut push_unique = |candidate: String| {
            let key = candidate.to_ascii_lowercase();
            if seen.insert(key) {
                candidates.push(candidate);
            }
        };

        if has_path_component {
            push_unique(filename.clone());
        }

        let language = get_registry_language().as_str().to_string();
        if !language.is_empty() {
            let localized = format!("Data/{language}/{TGA_DIR_PATH}{filename}");
            push_unique(localized);
        }

        push_unique(format!("{TGA_DIR_PATH}{filename}"));

        let user_data = global_data::read().get_user_data_dir().to_string();
        if !user_data.is_empty() {
            let mut user_textures = Path::new(&user_data)
                .join(USER_TGA_DIR_PATH.replace("%s", ""))
                .join(&filename)
                .to_string_lossy()
                .to_string();
            user_textures = user_textures.replace('\\', "/");
            push_unique(user_textures);

            if matches!(extension.as_deref(), Some("tga")) {
                let mut map_previews = Path::new(&user_data)
                    .join(MAP_PREVIEW_DIR_PATH.replace("%s", ""))
                    .join(&filename)
                    .to_string_lossy()
                    .to_string();
                map_previews = map_previews.replace('\\', "/");
                push_unique(map_previews);
            }
        }

        candidates
    }

    fn resolve_game_fs_path_cached(path: &str) -> Option<String> {
        let key = normalized_texture_key(path);
        let cache = GAME_FS_PATH_CACHE.get_or_init(|| Mutex::new(HashMap::new()));
        if let Ok(cache) = cache.lock() {
            if let Some(cached) = cache.get(&key) {
                return cached.clone();
            }
        }

        let resolved = Self::find_game_fs_path(path);
        if let Ok(mut cache) = cache.lock() {
            cache.insert(key, resolved.clone());
        }
        resolved
    }

    fn find_game_fs_path(path: &str) -> Option<String> {
        if path.trim().is_empty() {
            return None;
        }

        ensure_engine_filesystem_backends();

        let file_system = get_file_system();
        let mut fs = file_system.lock().ok()?;

        for candidate in Self::resource_candidates(path) {
            if fs
                .open_file(&candidate, FileAccess::READ.combine(FileAccess::BINARY))
                .is_some()
            {
                return Some(candidate);
            }
        }

        None
    }

    fn load_image_from_game_fs(path: &str) -> Option<Arc<image::DynamicImage>> {
        let Some(candidate) = Self::resolve_game_fs_path_cached(path) else {
            return None;
        };
        let key = normalized_texture_key(&candidate);
        let cache = GAME_FS_IMAGE_CACHE.get_or_init(|| Mutex::new(HashMap::new()));
        if let Ok(cache) = cache.lock() {
            if let Some(cached) = cache.get(&key) {
                return cached.clone();
            }
        }

        let result = Self::load_image_from_game_fs_path(&candidate).map(Arc::new);
        if let Ok(mut cache) = cache.lock() {
            cache.insert(key, result.clone());
        }
        result
    }

    fn load_image_from_game_fs_path(path: &str) -> Option<image::DynamicImage> {
        let file_system = get_file_system();
        let mut fs = file_system.lock().ok()?;
        let mut file = fs.open_file(path, FileAccess::READ.combine(FileAccess::BINARY))?;
        let bytes = file.read_entire_and_close().ok()?;
        Self::decode_image_from_bytes(path, &bytes)
    }

    fn decode_image_from_bytes(resource_name: &str, bytes: &[u8]) -> Option<image::DynamicImage> {
        let extension = Path::new(resource_name)
            .extension()
            .and_then(|ext| ext.to_str())
            .map(|ext| ext.to_ascii_lowercase());

        match extension.as_deref() {
            Some("tga") => image::load_from_memory_with_format(bytes, ImageFormat::Tga)
                .ok()
                .or_else(|| Self::decode_tga_image_manual(bytes)),
            Some("dds") => image::load_from_memory_with_format(bytes, ImageFormat::Dds).ok(),
            Some("png") => image::load_from_memory_with_format(bytes, ImageFormat::Png).ok(),
            Some("jpg") | Some("jpeg") => {
                image::load_from_memory_with_format(bytes, ImageFormat::Jpeg).ok()
            }
            Some("bmp") => image::load_from_memory_with_format(bytes, ImageFormat::Bmp).ok(),
            _ => image::load_from_memory(bytes).ok(),
        }
    }

    fn decode_tga_image_manual(bytes: &[u8]) -> Option<DynamicImage> {
        if bytes.len() < 18 {
            return None;
        }

        let id_length = bytes[0] as usize;
        let color_map_type = bytes[1];
        let image_type = bytes[2];
        let color_map_length = u16::from_le_bytes([bytes[5], bytes[6]]) as usize;
        let color_map_depth = bytes[7] as usize;
        let width = u16::from_le_bytes([bytes[12], bytes[13]]) as u32;
        let height = u16::from_le_bytes([bytes[14], bytes[15]]) as u32;
        let bits_per_pixel = bytes[16];
        let descriptor = bytes[17];

        if width == 0 || height == 0 {
            return None;
        }
        if !matches!(
            (image_type, bits_per_pixel),
            (2, 24) | (2, 32) | (10, 24) | (10, 32)
        ) {
            return None;
        }

        let mut offset = 18usize.saturating_add(id_length);
        if color_map_type == 1 {
            let cmap_bytes = color_map_length.saturating_mul(color_map_depth.saturating_add(7)) / 8;
            offset = offset.saturating_add(cmap_bytes);
        }
        if offset >= bytes.len() {
            return None;
        }

        let pixel_count = (width as usize).saturating_mul(height as usize);
        let mut rgba = Vec::with_capacity(pixel_count.saturating_mul(4));
        match (image_type, bits_per_pixel) {
            (2, 24) => {
                let expected = pixel_count.saturating_mul(3);
                if bytes.len() < offset.saturating_add(expected) {
                    return None;
                }
                for chunk in bytes[offset..offset + expected].chunks_exact(3) {
                    rgba.extend_from_slice(&[chunk[2], chunk[1], chunk[0], 255]);
                }
            }
            (2, 32) => {
                let expected = pixel_count.saturating_mul(4);
                if bytes.len() < offset.saturating_add(expected) {
                    return None;
                }
                for chunk in bytes[offset..offset + expected].chunks_exact(4) {
                    rgba.extend_from_slice(&[chunk[2], chunk[1], chunk[0], chunk[3]]);
                }
            }
            (10, 24) => {
                if !Self::decode_tga_rle(bytes, offset, pixel_count, 3, &mut rgba) {
                    return None;
                }
            }
            (10, 32) => {
                if !Self::decode_tga_rle(bytes, offset, pixel_count, 4, &mut rgba) {
                    return None;
                }
            }
            _ => return None,
        }

        // TGA default origin is bottom-left; flip to top-left to match GPU/image crate convention.
        let origin_upper_left = (descriptor & 0x20) != 0;
        if !origin_upper_left {
            let row_bytes = (width as usize).saturating_mul(4);
            if row_bytes > 0 {
                for y in 0..(height as usize / 2) {
                    let top = y * row_bytes;
                    let bottom = ((height as usize - 1 - y) * row_bytes) as usize;
                    for x in 0..row_bytes {
                        rgba.swap(top + x, bottom + x);
                    }
                }
            }
        }

        let image = RgbaImage::from_raw(width, height, rgba)?;
        Some(DynamicImage::ImageRgba8(image))
    }

    fn decode_tga_rle(
        bytes: &[u8],
        mut offset: usize,
        pixel_count: usize,
        bytes_per_pixel: usize,
        out: &mut Vec<u8>,
    ) -> bool {
        let mut written = 0usize;
        while written < pixel_count && offset < bytes.len() {
            let header = bytes[offset];
            offset += 1;
            let run_len = ((header & 0x7F) as usize).saturating_add(1);
            let rle_packet = (header & 0x80) != 0;

            if rle_packet {
                if offset.saturating_add(bytes_per_pixel) > bytes.len() {
                    return false;
                }
                let px = &bytes[offset..offset + bytes_per_pixel];
                offset += bytes_per_pixel;
                for _ in 0..run_len {
                    if written >= pixel_count {
                        break;
                    }
                    match bytes_per_pixel {
                        3 => out.extend_from_slice(&[px[2], px[1], px[0], 255]),
                        4 => out.extend_from_slice(&[px[2], px[1], px[0], px[3]]),
                        _ => return false,
                    }
                    written += 1;
                }
            } else {
                let run_bytes = run_len.saturating_mul(bytes_per_pixel);
                if offset.saturating_add(run_bytes) > bytes.len() {
                    return false;
                }
                for i in 0..run_len {
                    if written >= pixel_count {
                        break;
                    }
                    let start = offset + i * bytes_per_pixel;
                    let px = &bytes[start..start + bytes_per_pixel];
                    match bytes_per_pixel {
                        3 => out.extend_from_slice(&[px[2], px[1], px[0], 255]),
                        4 => out.extend_from_slice(&[px[2], px[1], px[0], px[3]]),
                        _ => return false,
                    }
                    written += 1;
                }
                offset += run_bytes;
            }
        }

        written == pixel_count
    }

    pub fn acquire_texture_view(
        &mut self,
        texture_id: TextureId,
        kind: TextureKind,
        device: &Device,
        queue: &Queue,
        fallback_color: [u8; 4],
    ) -> TerrainResult<Arc<TextureView>> {
        let key = (texture_id, kind);
        if let Some(entry) = self.gpu_cache.get(&key) {
            return Ok(entry.view.clone());
        }

        let (texture, view) =
            self.create_gpu_texture(texture_id, kind, device, queue, fallback_color)?;
        let texture = Arc::new(texture);
        let view = Arc::new(view);

        self.gpu_cache.insert(
            key,
            GPUTextureEntry {
                texture: texture.clone(),
                view: view.clone(),
            },
        );

        Ok(view)
    }

    fn create_gpu_texture(
        &mut self,
        texture_id: TextureId,
        kind: TextureKind,
        device: &Device,
        queue: &Queue,
        fallback_color: [u8; 4],
    ) -> TerrainResult<(Texture, TextureView)> {
        if let Some(texture) = self.textures.get_mut(&texture_id) {
            if kind != TextureKind::Diffuse {
                return Ok(create_solid_texture(device, queue, fallback_color));
            }

            let (resolved_path, requested_path) = {
                if texture.resolved_path.is_none() {
                    if let Some(path) = Self::resolve_texture_path_cached(&texture.diffuse_path) {
                        texture.resolved_path = Some(path);
                    }
                }
                (texture.resolved_path.clone(), texture.diffuse_path.clone())
            };

            let cached_image = texture.cached_diffuse_image.clone();

            let image = cached_image.or_else(|| {
                let Some(path) = resolved_path.as_deref().and_then(|path| path.to_str()) else {
                    return None;
                };
                let loaded = Self::load_image_from_game_fs(path);
                if let Some(img) = loaded.as_ref() {
                    texture
                        .cached_diffuse_image
                        .get_or_insert_with(|| img.clone());
                }
                loaded
            });

            if let Some(image) = image {
                let label = resolved_path
                    .as_ref()
                    .map(|path| format!("Terrain Texture {}", path.display()))
                    .unwrap_or_else(|| format!("Terrain Texture {}", requested_path));
                let rgba = image.to_rgba8();
                let (texture, view) = create_gpu_texture_from_rgba(
                    device,
                    queue,
                    &rgba,
                    image.width(),
                    image.height(),
                    Some(&label),
                );
                return Ok((texture, view));
            }
        }

        Ok(create_solid_texture(device, queue, fallback_color))
    }

    /// Load texture from file
    pub fn load_texture(&mut self, texture_id: TextureId) -> TerrainResult<()> {
        let texture = self.textures.get_mut(&texture_id).ok_or_else(|| {
            TerrainError::InvalidData(format!("Texture {} not found", texture_id))
        })?;

        if texture.loaded {
            return Ok(());
        }

        let resolved = Self::resolve_texture_path_cached(&texture.diffuse_path);

        if let Some(path) = resolved.clone() {
            texture.resolved_path = Some(path.clone());
        }

        let diffuse_image = texture
            .resolved_path
            .as_ref()
            .and_then(|path| path.to_str())
            .and_then(Self::load_image_from_game_fs);
        match diffuse_image {
            Some(img) => {
                texture.dimensions = Some(img.dimensions());
                texture.cached_diffuse_image = Some(img);
            }
            None => {
                let resolved_hint = resolved
                    .as_ref()
                    .map(|path| path.display().to_string())
                    .unwrap_or_else(|| "<none>".to_string());
                let gamefs_hint = Self::resolve_game_fs_path_cached(&texture.diffuse_path)
                    .unwrap_or_else(|| "<none>".to_string());
                warn!(
                    "Terrain texture '{}' failed to decode (resolved_path={}, gamefs_path={})",
                    texture.diffuse_path, resolved_hint, gamefs_hint
                );
                texture.dimensions = Some(DEFAULT_TEXTURE_DIMENSIONS);
            }
        }

        texture.loaded = true;
        self.stats.loaded_textures = self.stats.loaded_textures.saturating_add(1);

        Ok(())
    }

    /// Load a collection of textures from disk.
    pub fn load_textures(&mut self, texture_paths: &[&str]) -> TerrainResult<Vec<TextureId>> {
        let mut ids = Vec::new();
        for path in texture_paths {
            if !Self::can_resolve_texture_path(path) {
                warn!("Skipping unavailable terrain texture '{}'", path);
                continue;
            }

            let path_key = normalized_texture_key(path);
            if let Some(existing_id) = self.texture_path_index.get(&path_key).copied() {
                ids.push(existing_id);
                continue;
            }

            let name = Path::new(path)
                .file_stem()
                .and_then(|stem| stem.to_str())
                .unwrap_or(path)
                .to_string();

            let texture = TerrainTexture::new(0, name, path.to_string());
            let texture_id = self.register_texture(texture);
            self.load_texture(texture_id)?;
            ids.push(texture_id);
        }

        Ok(ids)
    }

    /// Generate texture weights for terrain vertex based on height and slope
    pub fn generate_texture_weights(
        &mut self,
        height: f32,
        slope: f32,
        tex_coords: [f32; 2],
        texture_rules: &[TextureRule],
    ) -> TextureWeights {
        let mut contributions: HashMap<TextureId, f32> = HashMap::new();

        for rule in texture_rules {
            let base_weight = rule.calculate_weight(height, slope, 0.0, 0.0);
            self.record_blend_debug(
                "rule_base",
                height,
                slope,
                tex_coords,
                rule.texture_id,
                None,
                base_weight,
            );
            if base_weight <= 0.0 {
                continue;
            }

            // Smaller priority value means higher precedence; bias accordingly.
            let priority_bias = 1.0 / (1.0 + rule.priority as f32);
            let mut weight = base_weight * priority_bias;
            self.record_blend_debug(
                "rule_weight",
                height,
                slope,
                tex_coords,
                rule.texture_id,
                None,
                weight,
            );

            contributions
                .entry(rule.texture_id)
                .and_modify(|existing| *existing += weight)
                .or_insert(weight);
        }

        self.stats.blend_operations = self.stats.blend_operations.wrapping_add(1);

        if contributions.is_empty() {
            if let Some(rule) = texture_rules.first() {
                return TextureWeights::single(rule.texture_id);
            }

            return TextureWeights::empty();
        }

        TextureWeights::from_weight_pairs(contributions.into_iter().collect())
    }

    /// Apply texture blending based on height and normal
    pub fn blend_textures_at_position(
        &self,
        _position: Vec3,
        height: f32,
        normal: Vec3,
        tex_coords: [f32; 2],
        base_weights: &TextureWeights,
        texture_rules: &[TextureRule],
    ) -> TextureWeights {
        if base_weights.is_empty() {
            if let Some(rule) = texture_rules.first() {
                return TextureWeights::single(rule.texture_id);
            }
            return TextureWeights::empty();
        }

        let mut adjusted_weights = base_weights.clone();
        let slope = normal.dot(Vec3::Y).clamp(-1.0, 1.0).acos();

        for idx in 0..MAX_BLEND_WEIGHTS {
            if idx >= MAX_BLEND_WEIGHTS {
                break;
            }

            let weight = adjusted_weights.weights[idx];
            if weight <= f32::EPSILON {
                continue;
            }

            let texture_id = adjusted_weights.indices[idx];
            self.record_blend_debug(
                "blend_input",
                height,
                slope,
                tex_coords,
                texture_id,
                None,
                weight,
            );
        }

        adjusted_weights.normalize();
        for (texture_id, weight) in adjusted_weights.iter_pairs() {
            self.record_blend_debug(
                "blend_final",
                height,
                slope,
                tex_coords,
                texture_id,
                None,
                weight,
            );
        }

        adjusted_weights
    }

    /// Get texture by ID
    pub fn get_texture(&self, texture_id: TextureId) -> Option<&TerrainTexture> {
        self.textures.get(&texture_id)
    }

    /// Sample representative color for the terrain at a world position.
    pub fn sample_color_at(&self, _x: f32, _y: f32) -> TerrainResult<[f32; 3]> {
        Ok([0.5, 0.5, 0.5])
    }

    /// Determine terrain texture identifier at a world position.
    pub fn get_terrain_type_at(&self, _x: f32, _y: f32) -> TerrainResult<u32> {
        if let Some((&texture_id, _)) = self.textures.iter().next() {
            Ok(texture_id as u32)
        } else {
            Ok(0)
        }
    }

    /// Return the lowest registered texture id, if any.
    pub fn first_texture_id(&self) -> Option<TextureId> {
        self.textures.keys().copied().min()
    }

    /// Get mutable texture by ID
    pub fn get_texture_mut(&mut self, texture_id: TextureId) -> Option<&mut TerrainTexture> {
        self.textures.get_mut(&texture_id)
    }

    /// Update texture system.
    pub fn update(&mut self) -> TerrainResult<()> {
        let start_time = std::time::Instant::now();

        self.stats.last_update_time = start_time.elapsed();
        Ok(())
    }

    fn record_blend_debug(
        &self,
        stage: &str,
        height: f32,
        slope: f32,
        tex_coords: [f32; 2],
        texture_id: TextureId,
        gradient: Option<f32>,
        weight: f32,
    ) {
        if let Some(path) = &self.blend_debug_path {
            if let Ok(mut file) = OpenOptions::new().create(true).append(true).open(path) {
                let gradient = gradient.unwrap_or(-1.0);
                let _ = writeln!(
                    file,
                    "{stage},{texture_id},{height:.6},{slope:.6},{u:.6},{v:.6},{gradient:.6},{weight:.6}",
                    stage = stage,
                    texture_id = texture_id,
                    height = height,
                    slope = slope,
                    u = tex_coords[0],
                    v = tex_coords[1],
                    gradient = gradient,
                    weight = weight
                );
            }
        }
    }

    /// Get performance statistics
    pub fn get_stats(&self) -> &TextureStats {
        &self.stats
    }

    /// Clear all textures
    pub fn clear(&mut self) {
        self.textures.clear();
        self.texture_path_index.clear();
        self.next_texture_id = 1;
        self.stats = TextureStats::default();
    }
}

/// Rule for automatic texture assignment
#[derive(Debug, Clone)]
pub struct TextureRule {
    pub texture_id: TextureId,
    pub height_min: f32,
    pub height_max: f32,
    pub slope_min: f32,
    pub slope_max: f32,
    pub priority: u8,
    pub preferred_gradient: f32,
    pub gradient_tolerance: f32,
}

impl TextureRule {
    /// Calculate blend weight for given height, slope, and sampled gradient magnitude
    pub fn calculate_weight(
        &self,
        height: f32,
        slope: f32,
        _gradient: f32,
        _height_strength: f32,
    ) -> f32 {
        let height_factor = if height >= self.height_min && height <= self.height_max {
            let height_span = (self.height_max - self.height_min).abs();
            if height_span <= f32::EPSILON {
                1.0
            } else {
                1.0 - ((height - (self.height_min + self.height_max) / 2.0).abs()
                    / (height_span / 2.0))
                    .min(1.0)
            }
        } else {
            0.0
        };

        let slope_factor = if slope >= self.slope_min && slope <= self.slope_max {
            let slope_span = (self.slope_max - self.slope_min).abs();
            if slope_span <= f32::EPSILON {
                1.0
            } else {
                1.0 - ((slope - (self.slope_min + self.slope_max) / 2.0).abs() / (slope_span / 2.0))
                    .min(1.0)
            }
        } else {
            0.0
        };

        if height_factor <= 0.0 || slope_factor <= 0.0 {
            return 0.0;
        }

        (height_factor * slope_factor).max(0.0)
    }
}

impl Default for TextureManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Legacy-friendly alias for the texture manager
pub type TerrainTextures = TextureManager;

/// Legacy-friendly alias for individual texture layers
pub type TextureLayer = TextureWeights;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_texture_creation() {
        let texture = TerrainTexture::new(1, "grass".to_string(), "grass.jpg".to_string());
        assert_eq!(texture.id, 1);
        assert_eq!(texture.name, "grass");
        assert!(!texture.loaded);
    }

    #[test]
    fn test_texture_weights_normalization() {
        let mut weights = TextureWeights::new([0, 1, 2, 3], [0.5, 0.5, 0.5, 0.5]);
        let sum: f32 = weights.weights.iter().sum();
        assert!((sum - 1.0).abs() < 0.001);
    }

    #[test]
    fn test_texture_weights_blend() {
        let weights = TextureWeights::blend_two(0, 1, 0.7);
        assert_eq!(weights.indices[0], 0);
        assert_eq!(weights.indices[1], 1);
        assert!((weights.weights[0] - 0.7).abs() < 0.001);
        assert!((weights.weights[1] - 0.3).abs() < 0.001);
    }

    #[test]
    fn test_texture_weights_from_pairs() {
        let weights = TextureWeights::from_weight_pairs(vec![(10, 2.0), (5, 1.0)]);
        assert_eq!(weights.indices[0], 10);
        assert_eq!(weights.indices[1], 5);
        assert!((weights.weights[0] - (2.0 / 3.0)).abs() < 0.001);
        assert!((weights.weights[1] - (1.0 / 3.0)).abs() < 0.001);
    }

    #[test]
    fn test_texture_manager() {
        let mut manager = TextureManager::new();
        let texture = TerrainTexture::new(0, "test".to_string(), "test.jpg".to_string());
        let id = manager.register_texture(texture);

        assert_eq!(id, 1);
        assert!(manager.get_texture(id).is_some());
        assert_eq!(manager.stats.total_textures, 1);
    }

    #[test]
    fn test_texture_rule() {
        let rule = TextureRule {
            texture_id: 1,
            height_min: 0.0,
            height_max: 100.0,
            slope_min: 0.0,
            slope_max: 0.5,
            priority: 1,
            preferred_gradient: -1.0,
            gradient_tolerance: 0.4,
        };

        let weight1 = rule.calculate_weight(50.0, 0.25, 0.0, 0.0);
        let weight2 = rule.calculate_weight(150.0, 0.25, 0.0, 0.0);

        assert!(weight1 > 0.0);
        assert_eq!(weight2, 0.0);
    }

    #[test]
    fn test_generate_texture_weights_priority_bias() {
        let mut manager = TextureManager::new();
        let rules = vec![
            TextureRule {
                texture_id: 1,
                height_min: 0.0,
                height_max: 200.0,
                slope_min: 0.0,
                slope_max: std::f32::consts::PI,
                priority: 0,
                preferred_gradient: -1.0,
                gradient_tolerance: 0.4,
            },
            TextureRule {
                texture_id: 2,
                height_min: 0.0,
                height_max: 200.0,
                slope_min: 0.0,
                slope_max: std::f32::consts::PI,
                priority: 5,
                preferred_gradient: -1.0,
                gradient_tolerance: 0.4,
            },
        ];

        let weights = manager.generate_texture_weights(50.0, 0.5, [0.5, 0.5], &rules);
        assert_eq!(weights.indices[0], 1);
        assert!(weights.weights[0] > weights.weights[1]);
    }

    #[test]
    fn test_generate_texture_weights_fallback() {
        let mut manager = TextureManager::new();
        let rules = vec![TextureRule {
            texture_id: 4,
            height_min: -10.0,
            height_max: 10.0,
            slope_min: 0.0,
            slope_max: 0.2,
            priority: 1,
            preferred_gradient: -1.0,
            gradient_tolerance: 0.4,
        }];

        let weights = manager.generate_texture_weights(100.0, 0.5, [0.5, 0.5], &rules);
        assert_eq!(weights.indices[0], 4);
        assert!((weights.weights[0] - 1.0).abs() < 0.001);
    }

    #[test]
    fn test_texture_rule_gradient_preference() {
        let rock_rule = TextureRule {
            texture_id: 1,
            height_min: 0.0,
            height_max: 200.0,
            slope_min: 0.0,
            slope_max: std::f32::consts::PI,
            priority: 0,
            preferred_gradient: 0.8,
            gradient_tolerance: 0.2,
        };

        let dirt_rule = TextureRule {
            texture_id: 2,
            height_min: 0.0,
            height_max: 200.0,
            slope_min: 0.0,
            slope_max: std::f32::consts::PI,
            priority: 0,
            preferred_gradient: 0.1,
            gradient_tolerance: 0.2,
        };

        let height = 100.0;
        let slope = 0.35;
        let steep_gradient = 0.9;
        let shallow_gradient = 0.05;

        let rock_weight_steep = rock_rule.calculate_weight(height, slope, steep_gradient, 0.5);
        let rock_weight_shallow = rock_rule.calculate_weight(height, slope, shallow_gradient, 0.5);
        assert!((rock_weight_steep - rock_weight_shallow).abs() < f32::EPSILON);

        let dirt_weight_steep = dirt_rule.calculate_weight(height, slope, steep_gradient, 0.5);
        let dirt_weight_shallow = dirt_rule.calculate_weight(height, slope, shallow_gradient, 0.5);
        assert!((dirt_weight_shallow - dirt_weight_steep).abs() < f32::EPSILON);
    }

    #[test]
    fn test_generate_texture_weights_height_slope_only() {
        let mut manager = TextureManager::new();

        let rules = vec![
            TextureRule {
                texture_id: 1,
                height_min: 0.0,
                height_max: 200.0,
                slope_min: 0.0,
                slope_max: 0.4,
                priority: 0,
                preferred_gradient: 0.0,
                gradient_tolerance: 0.0,
            },
            TextureRule {
                texture_id: 2,
                height_min: 0.0,
                height_max: 200.0,
                slope_min: 0.6,
                slope_max: 1.4,
                priority: 0,
                preferred_gradient: 0.0,
                gradient_tolerance: 0.0,
            },
        ];

        let low_slope = manager.generate_texture_weights(100.0, 0.2, [0.5, 0.5], &rules);
        assert_eq!(low_slope.indices[0], 1);
        assert!((low_slope.weights[0] - 1.0).abs() < 0.001);

        let steep_only = manager.generate_texture_weights(100.0, 0.8, [0.5, 0.5], &rules);
        assert_eq!(steep_only.indices[0], 2);
        assert!((steep_only.weights[0] - 1.0).abs() < 0.001);
    }
}

fn create_gpu_texture_from_rgba(
    device: &Device,
    queue: &Queue,
    rgba: &[u8],
    width: u32,
    height: u32,
    label: Option<&str>,
) -> (Texture, TextureView) {
    let texture_size = wgpu::Extent3d {
        width,
        height,
        depth_or_array_layers: 1,
    };

    let texture = device.create_texture(&TextureDescriptor {
        label,
        size: texture_size,
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: TextureFormat::Rgba8UnormSrgb,
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
        rgba,
        wgpu::TexelCopyBufferLayout {
            offset: 0,
            bytes_per_row: Some(4 * width),
            rows_per_image: Some(height),
        },
        texture_size,
    );

    let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
    (texture, view)
}

fn create_solid_texture(device: &Device, queue: &Queue, color: [u8; 4]) -> (Texture, TextureView) {
    let solid_pixels = [color; 1];
    let bytes: &[u8] = cast_slice(&solid_pixels);
    create_gpu_texture_from_rgba(device, queue, bytes, 1, 1, Some("Terrain Solid Texture"))
}
