//! Terrain Texture System
//!
//! Manages terrain texturing including texture atlases, blending, and
//! material properties for realistic terrain rendering.

use crate::terrain::{TerrainError, TerrainResult};
use bytemuck::cast_slice;
use game_engine::common::ascii_string::AsciiString;
use game_engine::common::ini::ini_terrain::get_terrain_types;
use game_engine::common::system::{file::FileAccess, file_system::get_file_system};
use glam::{Vec2, Vec3};
use image::{self, GenericImageView};
use log::warn;
use std::collections::HashMap;
use std::env;
use std::fs::{File, OpenOptions};
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::sync::{Arc, OnceLock};
use wgpu::{Device, Queue, Texture, TextureDescriptor, TextureFormat, TextureView};

/// Unique identifier for terrain textures
pub type TextureId = u32;

/// Maximum blend weights tracked per vertex (matches legacy limit)
pub const MAX_BLEND_WEIGHTS: usize = 4;
pub const DEFAULT_TEXTURE_DIMENSIONS: (u32, u32) = (256, 256);
pub const DEFAULT_NORMAL_COLOR: [u8; 4] = [128, 128, 255, 255];
pub const DEFAULT_HEIGHT_COLOR: [u8; 4] = [128, 128, 128, 255];

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum TextureKind {
    Diffuse,
    Normal,
    Height,
}

type TextureCacheKey = (TextureId, TextureKind);

/// Terrain texture with material properties
#[derive(Debug, Clone)]
pub struct TerrainTexture {
    /// Unique identifier
    pub id: TextureId,

    /// Display name for editor/debugging
    pub name: String,

    /// Path to diffuse texture
    pub diffuse_path: String,

    /// Path to normal map (optional)
    pub normal_path: Option<String>,

    /// Path to height/displacement map (optional)
    pub height_path: Option<String>,

    /// Path to material properties map (optional)
    pub material_path: Option<String>,

    /// Texture tiling scale
    pub scale: Vec2,

    /// Texture rotation in radians
    pub rotation: f32,

    /// Material properties
    pub material: TerrainMaterial,

    /// Blending properties
    pub blend_mode: BlendMode,

    /// Whether texture is loaded
    pub loaded: bool,

    /// Texture dimensions (when loaded)
    pub dimensions: Option<(u32, u32)>,

    /// Resolved on-disk path for GPU loading
    pub resolved_path: Option<PathBuf>,

    /// Resolved normal map path
    pub resolved_normal_path: Option<PathBuf>,

    /// Resolved height map path
    pub resolved_height_path: Option<PathBuf>,

    /// Cached height image data for gradient sampling
    pub height_image: Option<Arc<HeightImage>>,
}

/// Material properties for terrain textures
#[derive(Debug, Clone)]
pub struct TerrainMaterial {
    /// Base color/albedo tint
    pub albedo: [f32; 3],

    /// Metallic factor (0.0 = dielectric, 1.0 = metallic)
    pub metallic: f32,

    /// Roughness factor (0.0 = mirror, 1.0 = completely rough)
    pub roughness: f32,

    /// Normal map intensity
    pub normal_strength: f32,

    /// Height/displacement strength
    pub height_strength: f32,

    /// Ambient occlusion strength
    pub ao_strength: f32,

    /// Specular reflectance for dielectric materials
    pub specular: f32,
}

/// Texture blending modes
#[derive(Debug, Clone, PartialEq)]
pub enum BlendMode {
    /// Replace underlying texture
    Replace,
    /// Alpha blend with underlying texture
    AlphaBlend,
    /// Multiply with underlying texture
    Multiply,
    /// Additive blend
    Add,
    /// Height-based blending
    HeightBlend,
    /// Normal-based blending (blend based on surface slope)
    SlopeBlend,
}

/// Texture atlas for efficient GPU usage
#[derive(Debug)]
pub struct TextureAtlas {
    /// Atlas identifier
    pub id: u32,

    /// Atlas dimensions
    pub width: u32,
    pub height: u32,

    /// Individual texture regions in atlas
    pub regions: HashMap<TextureId, AtlasRegion>,

    /// Whether atlas needs rebuilding
    pub dirty: bool,

    /// Atlas texture data (RGBA)
    pub data: Option<Vec<u8>>,
}

/// Region within texture atlas
#[derive(Debug, Clone)]
pub struct AtlasRegion {
    /// Texture ID this region represents
    pub texture_id: TextureId,

    /// UV coordinates in atlas space [0,1]
    pub uv_min: Vec2,
    pub uv_max: Vec2,

    /// Original texture dimensions
    pub original_size: (u32, u32),
}

/// Texture blending weights for terrain vertices
#[derive(Debug, Clone)]
pub struct TextureWeights {
    /// Up to 4 texture identifiers
    pub indices: [TextureId; 4],

    /// Corresponding blend weights (should sum to 1.0)
    pub weights: [f32; 4],
}

/// Configuration for texture blending
#[derive(Debug, Clone)]
pub struct BlendConfig {
    /// Maximum number of textures per vertex
    pub max_textures_per_vertex: u8,

    /// Blend sharpness (higher = sharper transitions)
    pub blend_sharpness: f32,

    /// Height blend contrast
    pub height_contrast: f32,

    /// Slope blend angle threshold (radians)
    pub slope_threshold: f32,
}

/// Manages terrain textures and blending
#[derive(Debug)]
pub struct TextureManager {
    /// All registered textures
    textures: HashMap<TextureId, TerrainTexture>,

    /// Texture atlases for GPU efficiency
    atlases: HashMap<u32, TextureAtlas>,

    /// GPU texture cache keyed by texture identifier
    gpu_cache: HashMap<TextureCacheKey, GPUTextureEntry>,

    /// Next available texture ID
    next_texture_id: TextureId,

    /// Next available atlas ID
    next_atlas_id: u32,

    /// Blend configuration
    blend_config: BlendConfig,

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
    pub total_atlases: u32,
    pub texture_memory: u64,
    pub atlas_memory: u64,
    pub blend_operations: u64,
    pub last_update_time: std::time::Duration,
}

#[derive(Debug, Clone)]
struct GPUTextureEntry {
    texture: Arc<Texture>,
    view: Arc<TextureView>,
}

#[derive(Debug)]
pub struct HeightImage {
    width: u32,
    height: u32,
    data: Vec<u8>,
}

impl Default for TerrainMaterial {
    fn default() -> Self {
        Self {
            albedo: [1.0, 1.0, 1.0],
            metallic: 0.0,
            roughness: 0.8,
            normal_strength: 1.0,
            height_strength: 0.02,
            ao_strength: 1.0,
            specular: 0.04,
        }
    }
}

impl Default for BlendConfig {
    fn default() -> Self {
        Self {
            max_textures_per_vertex: 4,
            blend_sharpness: 8.0,
            height_contrast: 0.1,
            slope_threshold: 0.7854, // 45 degrees
        }
    }
}

impl TerrainTexture {
    /// Create new terrain texture
    pub fn new(id: TextureId, name: String, diffuse_path: String) -> Self {
        Self {
            id,
            name,
            diffuse_path,
            normal_path: None,
            height_path: None,
            material_path: None,
            scale: Vec2::new(1.0, 1.0),
            rotation: 0.0,
            material: TerrainMaterial::default(),
            blend_mode: BlendMode::AlphaBlend,
            loaded: false,
            dimensions: None,
            resolved_path: None,
            resolved_normal_path: None,
            resolved_height_path: None,
            height_image: None,
        }
    }

    /// Set normal map path
    pub fn with_normal_map<P: AsRef<Path>>(mut self, path: P) -> Self {
        self.normal_path = Some(path.as_ref().to_string_lossy().to_string());
        self
    }

    /// Set height map path
    pub fn with_height_map<P: AsRef<Path>>(mut self, path: P) -> Self {
        self.height_path = Some(path.as_ref().to_string_lossy().to_string());
        self
    }

    /// Set material properties
    pub fn with_material(mut self, material: TerrainMaterial) -> Self {
        self.material = material;
        self
    }

    /// Set texture scale
    pub fn with_scale(mut self, scale: Vec2) -> Self {
        self.scale = scale;
        self
    }

    /// Set blend mode
    pub fn with_blend_mode(mut self, blend_mode: BlendMode) -> Self {
        self.blend_mode = blend_mode;
        self
    }

    /// Check if texture has all required maps loaded
    pub fn is_complete(&self) -> bool {
        self.loaded && self.dimensions.is_some()
    }

    /// Get texture UV coordinates with scale and rotation applied
    pub fn transform_uv(&self, uv: Vec2) -> Vec2 {
        // Apply scale
        let scaled_uv = Vec2::new(uv.x * self.scale.x, uv.y * self.scale.y);

        // Apply rotation
        if self.rotation != 0.0 {
            let cos_r = self.rotation.cos();
            let sin_r = self.rotation.sin();
            Vec2::new(
                scaled_uv.x * cos_r - scaled_uv.y * sin_r,
                scaled_uv.x * sin_r + scaled_uv.y * cos_r,
            )
        } else {
            scaled_uv
        }
    }
}

impl TextureAtlas {
    /// Create new texture atlas
    pub fn new(id: u32, width: u32, height: u32) -> Self {
        Self {
            id,
            width,
            height,
            regions: HashMap::new(),
            dirty: true,
            data: None,
        }
    }

    /// Add texture region to atlas
    pub fn add_region(&mut self, texture_id: TextureId, region: AtlasRegion) {
        self.regions.insert(texture_id, region);
        self.dirty = true;
    }

    /// Get UV coordinates for texture in atlas
    pub fn get_texture_uv(&self, texture_id: TextureId, original_uv: Vec2) -> Option<Vec2> {
        if let Some(region) = self.regions.get(&texture_id) {
            // Remap UV from [0,1] to atlas region
            let u = region.uv_min.x + (region.uv_max.x - region.uv_min.x) * original_uv.x;
            let v = region.uv_min.y + (region.uv_max.y - region.uv_min.y) * original_uv.y;
            Some(Vec2::new(u, v))
        } else {
            None
        }
    }

    /// Rebuild atlas from constituent textures.
    pub fn rebuild(&mut self, textures: &HashMap<TextureId, TerrainTexture>) -> TerrainResult<()> {
        let mut atlas_data = vec![0u8; (self.width * self.height * 4) as usize];

        for region in self.regions.values() {
            let Some(texture) = textures.get(&region.texture_id) else {
                continue;
            };

            let path = texture
                .resolved_path
                .as_ref()
                .map(|p| p.as_path())
                .unwrap_or_else(|| Path::new(&texture.diffuse_path));

            let Ok(img) = image::open(path) else {
                warn!("TextureAtlas: unable to open {}", path.display());
                continue;
            };

            let region_x = (region.uv_min.x * self.width as f32).round() as u32;
            let region_y = (region.uv_min.y * self.height as f32).round() as u32;
            let region_w = ((region.uv_max.x - region.uv_min.x) * self.width as f32).round() as u32;
            let region_h =
                ((region.uv_max.y - region.uv_min.y) * self.height as f32).round() as u32;

            if region_w == 0 || region_h == 0 {
                continue;
            }

            let resized = image::imageops::resize(
                &img.to_rgba8(),
                region_w,
                region_h,
                image::imageops::FilterType::Nearest,
            );
            let raw = resized.into_raw();

            for y in 0..region_h {
                let dest_y = region_y + y;
                if dest_y >= self.height {
                    continue;
                }
                let dest_row = dest_y as usize * self.width as usize * 4;
                let src_row = y as usize * region_w as usize * 4;
                for x in 0..region_w {
                    let dest_x = region_x + x;
                    if dest_x >= self.width {
                        continue;
                    }
                    let dest_idx = dest_row + dest_x as usize * 4;
                    let src_idx = src_row + x as usize * 4;
                    atlas_data[dest_idx..dest_idx + 4].copy_from_slice(&raw[src_idx..src_idx + 4]);
                }
            }
        }

        self.data = Some(atlas_data);
        self.dirty = false;
        Ok(())
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
            return Self::single(0);
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
            atlases: HashMap::new(),
            gpu_cache: HashMap::new(),
            next_texture_id: 1,
            next_atlas_id: 1,
            blend_config: BlendConfig::default(),
            stats: TextureStats::default(),
            blend_debug_path,
        }
    }

    /// Reset all textures and atlases back to defaults
    pub fn reset(&mut self) -> TerrainResult<()> {
        self.textures.clear();
        self.atlases.clear();
        self.gpu_cache.clear();
        self.next_texture_id = 1;
        self.next_atlas_id = 1;
        self.stats = TextureStats::default();
        Ok(())
    }

    /// Register new terrain texture
    pub fn register_texture(&mut self, mut texture: TerrainTexture) -> TextureId {
        texture.id = self.next_texture_id;
        let id = texture.id;

        self.textures.insert(id, texture);
        self.next_texture_id += 1;
        self.stats.total_textures += 1;

        id
    }

    pub fn resolve_texture_path(diffuse_path: &str) -> Option<PathBuf> {
        let raw = Path::new(diffuse_path);
        let mut candidates = Vec::new();

        if raw.is_absolute() {
            candidates.push(raw.to_path_buf());
        }

        for resource in Self::resource_candidates(diffuse_path) {
            candidates.push(PathBuf::from(&resource));

            if let Ok(current_dir) = env::current_dir() {
                candidates.push(current_dir.join(&resource));
                for ancestor in current_dir.ancestors() {
                    candidates.push(ancestor.join(&resource));
                }
            }
        }

        const FALLBACK_DIRS: [&str; 12] = [
            "windows_game/extracted_big_files",
            "windows_game/extracted_big_files_v2",
            "windows_game/extracted_big_files/TexturesZH/Art/Textures",
            "windows_game/extracted_big_files_v2/TexturesZH/Art/Textures",
            "windows_game/extracted_big_files/TexturesZH/Art/Terrain",
            "windows_game/extracted_big_files_v2/TexturesZH/Art/Terrain",
            "windows_game/extracted_big_files/TerrainZH/Art/Terrain",
            "windows_game/extracted_big_files_v2/TerrainZH/Art/Terrain",
            "../windows_game/extracted_big_files/TexturesZH/Art/Textures",
            "../windows_game/extracted_big_files_v2/TexturesZH/Art/Textures",
            "../windows_game/extracted_big_files/TerrainZH/Art/Terrain",
            "../windows_game/extracted_big_files_v2/TerrainZH/Art/Terrain",
        ];

        for base in FALLBACK_DIRS.iter() {
            for resource in Self::resource_candidates(diffuse_path) {
                candidates.push(Path::new(base).join(resource));
            }
        }

        for candidate in candidates {
            if candidate.exists() {
                return Some(candidate);
            }
        }

        None
    }

    fn resource_candidates(filename: &str) -> Vec<String> {
        let normalized = filename.replace('\\', "/");
        let bare = normalized.trim_start_matches("./").to_string();
        let extension = Path::new(&bare)
            .extension()
            .and_then(|ext| ext.to_str())
            .map(|ext| ext.to_ascii_lowercase());
        let has_extension = extension.is_some();
        let mut candidates = Vec::new();

        fn push_unique(candidates: &mut Vec<String>, candidate: String) {
            if !candidate.is_empty() && !candidates.iter().any(|existing| existing == &candidate) {
                candidates.push(candidate);
            }
        }

        if let Some(alias) = Self::terrain_texture_alias(&bare) {
            push_unique(&mut candidates, alias.to_string());
        }

        push_unique(&mut candidates, bare.clone());

        for class_candidate in Self::terrain_class_candidates(&bare) {
            push_unique(&mut candidates, class_candidate);
        }

        for available_candidate in Self::terrain_available_family_candidates(&bare) {
            push_unique(&mut candidates, available_candidate);
        }

        for prefix_candidate in Self::terrain_prefix_fallback_candidates(&bare) {
            push_unique(&mut candidates, prefix_candidate);
        }

        if !bare.starts_with("Data/") {
            push_unique(&mut candidates, format!("Data/{bare}"));
        }

        if !bare.contains('/') {
            push_unique(&mut candidates, format!("Art/Terrain/{bare}"));
            push_unique(&mut candidates, format!("Art/Textures/{bare}"));
            push_unique(&mut candidates, format!("Art/W3D/{bare}"));
            push_unique(&mut candidates, format!("Data/Art/Terrain/{bare}"));
            push_unique(&mut candidates, format!("Data/Art/Textures/{bare}"));
            push_unique(&mut candidates, format!("Data/Art/W3D/{bare}"));
        }

        if has_extension {
            if let Some((base, ext)) = bare.rsplit_once('.') {
                let alt_exts: &[&str] = match extension.as_deref() {
                    Some("tga") => &["dds", "png", "bmp", "jpg", "jpeg"],
                    Some("dds") => &["tga", "png", "bmp", "jpg", "jpeg"],
                    Some("png") => &["tga", "dds", "bmp", "jpg", "jpeg"],
                    Some("bmp") => &["tga", "dds", "png", "jpg", "jpeg"],
                    Some("jpg") | Some("jpeg") => &["tga", "dds", "png", "bmp"],
                    _ => &["tga", "dds", "png", "jpg", "jpeg", "bmp"],
                };
                let seeds = candidates.clone();
                for seed in &seeds {
                    let seed_base = seed.strip_suffix(&format!(".{ext}")).unwrap_or(seed);
                    for alt in alt_exts {
                        push_unique(&mut candidates, format!("{seed_base}.{alt}"));
                    }
                }
                push_unique(&mut candidates, base.to_string());
            }
        } else {
            let seeds = candidates.clone();
            for seed in &seeds {
                for ext in ["tga", "dds", "png", "jpg", "jpeg", "bmp"] {
                    push_unique(&mut candidates, format!("{seed}.{ext}"));
                }
            }
        }

        if let Some((variant_prefix, variant_ext, sibling_variants)) =
            Self::terrain_variant_family(&bare)
        {
            let seeds = candidates.clone();
            for seed in &seeds {
                let seed_base = seed
                    .strip_suffix(&format!(".{variant_ext}"))
                    .unwrap_or(seed);
                let stem = Path::new(seed_base)
                    .file_name()
                    .and_then(|name| name.to_str())
                    .unwrap_or(seed_base);
                if !stem
                    .to_ascii_lowercase()
                    .starts_with(&variant_prefix.to_ascii_lowercase())
                {
                    continue;
                }
                let stem_without_variant = &seed_base[..seed_base.len().saturating_sub(1)];
                for sibling in &sibling_variants {
                    push_unique(
                        &mut candidates,
                        format!("{stem_without_variant}{sibling}.{variant_ext}"),
                    );
                }
            }
        }

        if let Some((family_prefix, family_number, family_suffix, family_ext)) =
            Self::terrain_numeric_family(&bare)
        {
            let seeds = candidates.clone();
            for seed in &seeds {
                let seed_base = seed.strip_suffix(&format!(".{family_ext}")).unwrap_or(seed);
                let stem = Path::new(seed_base)
                    .file_name()
                    .and_then(|name| name.to_str())
                    .unwrap_or(seed_base);
                if !stem
                    .to_ascii_lowercase()
                    .starts_with(&family_prefix.to_ascii_lowercase())
                {
                    continue;
                }
                let stem_prefix =
                    &seed_base[..seed_base.len().saturating_sub(family_suffix.len() + 2)];
                for distance in 1_i32..=20 {
                    for delta in [distance, -distance] {
                        let candidate_number = family_number as i32 + delta;
                        if !(0..=99).contains(&candidate_number) {
                            continue;
                        }
                        push_unique(
                            &mut candidates,
                            format!(
                                "{stem_prefix}{candidate_number:02}{family_suffix}.{family_ext}"
                            ),
                        );
                    }
                }
            }
        }

        candidates
    }

    fn terrain_texture_alias(filename: &str) -> Option<&'static str> {
        match filename.to_ascii_lowercase().as_str() {
            // The extracted asset set used by the Rust shell path is missing this exact
            // terrain tile payload even though Terrain.ini still references it.
            // Use the adjacent rock tile family as a fallback alias.
            "txrock07a.tga" => Some("TXRock06a.tga"),
            _ => None,
        }
    }

    fn terrain_class_candidates(filename: &str) -> Vec<String> {
        let Some(registry) = get_terrain_types() else {
            return Vec::new();
        };

        let target = filename.replace('\\', "/").to_ascii_lowercase();
        let guard = registry.read();
        let mut matched_class: Option<String> = None;
        let mut matched_surface = None;

        for terrain_name in guard.get_terrain_names() {
            let Some(terrain) = guard.find_terrain(&AsciiString::from(terrain_name.as_str()))
            else {
                continue;
            };
            if terrain
                .texture_name
                .as_str()
                .replace('\\', "/")
                .to_ascii_lowercase()
                == target
            {
                matched_class = terrain.properties.get("Class").cloned();
                matched_surface = Some(terrain.surface_type.clone());
                break;
            }
        }

        let Some(surface) = matched_surface else {
            return Vec::new();
        };

        let mut class_matches = Vec::new();
        let mut surface_matches = Vec::new();

        for terrain_name in guard.get_terrain_names() {
            let Some(terrain) = guard.find_terrain(&AsciiString::from(terrain_name.as_str()))
            else {
                continue;
            };
            let texture_name = terrain.texture_name.as_str().trim();
            if texture_name.is_empty() {
                continue;
            }
            let normalized = texture_name.replace('\\', "/");
            if normalized.eq_ignore_ascii_case(filename) {
                continue;
            }

            if let Some(class_name) = matched_class.as_ref() {
                if terrain
                    .properties
                    .get("Class")
                    .map(|value| value.eq_ignore_ascii_case(class_name))
                    .unwrap_or(false)
                {
                    class_matches.push(normalized.clone());
                    continue;
                }
            }

            if terrain.surface_type == surface {
                surface_matches.push(normalized);
            }
        }

        class_matches.sort();
        class_matches.dedup_by(|a, b| a.eq_ignore_ascii_case(b));
        surface_matches.sort();
        surface_matches.dedup_by(|a, b| a.eq_ignore_ascii_case(b));
        class_matches.extend(surface_matches);
        class_matches
    }

    fn terrain_available_family_candidates(filename: &str) -> Vec<String> {
        let Some((prefix, number, suffix, ext)) = Self::terrain_numeric_family(filename) else {
            return Vec::new();
        };

        let mut matches: Vec<(i32, String)> = Vec::new();
        for available in Self::available_terrain_textures() {
            let available_name = Path::new(available)
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or(available.as_str());
            let Some((candidate_prefix, candidate_number, candidate_suffix, candidate_ext)) =
                Self::terrain_numeric_family(available_name)
            else {
                continue;
            };

            if !candidate_prefix.eq_ignore_ascii_case(&prefix) {
                continue;
            }
            if !candidate_ext.eq_ignore_ascii_case(&ext) {
                continue;
            }

            let mut score = (candidate_number as i32 - number as i32).abs();
            if !candidate_suffix.eq_ignore_ascii_case(&suffix) {
                score += 100;
            }

            matches.push((score, available_name.to_string()));
        }

        matches.sort_by(|a, b| a.0.cmp(&b.0).then_with(|| a.1.cmp(&b.1)));
        matches.dedup_by(|a, b| a.1.eq_ignore_ascii_case(&b.1));
        matches.into_iter().map(|(_, name)| name).take(8).collect()
    }

    fn terrain_prefix_fallback_candidates(filename: &str) -> Vec<String> {
        let stem = Path::new(filename)
            .file_stem()
            .and_then(|stem| stem.to_str())
            .unwrap_or(filename)
            .to_ascii_lowercase();

        let preferred = if stem.starts_with("tmgras") || stem.starts_with("tlgras") {
            &["TMGras37a.tga", "TGGrass02.tga"][..]
        } else if stem.starts_with("tmcliff")
            || stem.starts_with("tlcliff")
            || stem.starts_with("txrock")
            || stem.starts_with("tmrock")
        {
            &["rock01.tga", "TLSandstone01.tga"][..]
        } else if stem.starts_with("tmsand")
            || stem.starts_with("txsand")
            || stem.starts_with("tlsand")
            || stem.starts_with("tmdirt")
            || stem.starts_with("tldirt")
        {
            &["TGGrcSand01.tga", "TLSandstone01.tga", "TGGrass02.tga"][..]
        } else {
            &[
                "TGGrass02.tga",
                "TGGrcSand01.tga",
                "rock01.tga",
                "TLSandstone01.tga",
            ][..]
        };

        preferred
            .iter()
            .filter(|candidate| {
                Self::available_terrain_textures()
                    .iter()
                    .any(|available| available.eq_ignore_ascii_case(candidate))
            })
            .map(|candidate| (*candidate).to_string())
            .collect()
    }

    fn available_terrain_textures() -> &'static Vec<String> {
        static AVAILABLE_TERRAIN_TEXTURES: OnceLock<Vec<String>> = OnceLock::new();
        AVAILABLE_TERRAIN_TEXTURES.get_or_init(|| {
            let search_dirs = [
                "windows_game/extracted_big_files/TerrainZH/Art/Terrain",
                "windows_game/extracted_big_files_v2/TerrainZH/Art/Terrain",
                "windows_game/extracted_big_files/TexturesZH/Art/Terrain",
                "windows_game/extracted_big_files_v2/TexturesZH/Art/Terrain",
                "../windows_game/extracted_big_files/TerrainZH/Art/Terrain",
                "../windows_game/extracted_big_files_v2/TerrainZH/Art/Terrain",
                "../windows_game/extracted_big_files/TexturesZH/Art/Terrain",
                "../windows_game/extracted_big_files_v2/TexturesZH/Art/Terrain",
            ];

            let mut available = Vec::new();
            for dir in search_dirs {
                let path = Path::new(dir);
                let Ok(entries) = std::fs::read_dir(path) else {
                    continue;
                };
                for entry in entries.flatten() {
                    let candidate = entry.path();
                    if !candidate.is_file() {
                        continue;
                    }
                    let Some(name) = candidate.file_name().and_then(|name| name.to_str()) else {
                        continue;
                    };
                    if !name.contains('.') {
                        continue;
                    }
                    if !available
                        .iter()
                        .any(|existing: &String| existing.eq_ignore_ascii_case(name))
                    {
                        available.push(name.to_string());
                    }
                }
            }

            available.sort();
            available
        })
    }

    fn terrain_variant_family(filename: &str) -> Option<(String, String, Vec<char>)> {
        let path = Path::new(filename);
        let ext = path.extension()?.to_str()?.to_ascii_lowercase();
        let stem = path.file_stem()?.to_str()?;
        let mut chars = stem.chars();
        let last = chars.next_back()?;
        let prev = chars.next_back()?;
        if !last.is_ascii_alphabetic() || !prev.is_ascii_digit() {
            return None;
        }

        let variant_prefix = stem[..stem.len().saturating_sub(1)].to_string();
        let mut siblings = Vec::new();
        for candidate in ['a', 'b', 'c', 'd'] {
            if candidate != last.to_ascii_lowercase() {
                siblings.push(candidate);
            }
        }
        Some((variant_prefix, ext, siblings))
    }

    fn terrain_numeric_family(filename: &str) -> Option<(String, u8, String, String)> {
        let path = Path::new(filename);
        let ext = path.extension()?.to_str()?.to_ascii_lowercase();
        let stem = path.file_stem()?.to_str()?;
        let chars: Vec<char> = stem.chars().collect();
        if chars.len() < 3 {
            return None;
        }

        let mut digit_end = chars.len();
        while digit_end > 0 && !chars[digit_end - 1].is_ascii_digit() {
            digit_end -= 1;
        }
        if digit_end < 2 {
            return None;
        }

        let mut digit_start = digit_end;
        while digit_start > 0 && chars[digit_start - 1].is_ascii_digit() {
            digit_start -= 1;
        }
        let digits = &stem[digit_start..digit_end];
        if digits.len() != 2 {
            return None;
        }
        let number = digits.parse::<u8>().ok()?;
        let prefix = stem[..digit_start].to_string();
        let suffix = stem[digit_end..].to_string();
        if prefix.is_empty() {
            return None;
        }
        Some((prefix, number, suffix, ext))
    }

    fn load_image_from_game_fs(path: &str) -> Option<image::DynamicImage> {
        let file_system = get_file_system();
        let mut fs = file_system.lock().ok()?;

        for candidate in Self::resource_candidates(path) {
            let Some(mut file) =
                fs.open_file(&candidate, FileAccess::READ.combine(FileAccess::BINARY))
            else {
                continue;
            };
            let Ok(bytes) = file.read_entire_and_close() else {
                continue;
            };
            if let Ok(image) = image::load_from_memory(&bytes) {
                return Some(image);
            }
        }

        for archive in Self::archive_candidates() {
            for candidate in Self::resource_candidates(path) {
                if let Ok(Some(bytes)) = Self::extract_big_entry(&archive, &[candidate.as_str()]) {
                    if let Ok(image) = image::load_from_memory(&bytes) {
                        return Some(image);
                    }
                }
            }
        }

        None
    }

    fn archive_candidates() -> Vec<PathBuf> {
        let relatives = [
            "windows_game/Command & Conquer Generals Zero Hour/TerrainZH.big",
            "windows_game/Command & Conquer Generals Zero Hour/TexturesZH.big",
            "windows_game/Command & Conquer Generals Zero Hour/W3DZH.big",
            "../windows_game/Command & Conquer Generals Zero Hour/TerrainZH.big",
            "../windows_game/Command & Conquer Generals Zero Hour/TexturesZH.big",
            "../windows_game/Command & Conquer Generals Zero Hour/W3DZH.big",
        ];

        let mut candidates = Vec::new();
        for relative in relatives {
            let path = PathBuf::from(relative);
            if !candidates.iter().any(|existing| existing == &path) {
                candidates.push(path.clone());
            }

            if let Ok(current_dir) = env::current_dir() {
                for ancestor in current_dir.ancestors() {
                    let candidate = ancestor.join(relative);
                    if !candidates.iter().any(|existing| existing == &candidate) {
                        candidates.push(candidate);
                    }
                }
            }
        }

        candidates
    }

    fn extract_big_entry(
        candidate: &Path,
        entry_names: &[&str],
    ) -> std::io::Result<Option<Vec<u8>>> {
        let mut file = File::open(candidate)?;

        let mut magic = [0u8; 4];
        file.read_exact(&mut magic)?;
        if magic != *b"BIGF" && magic != *b"BIG4" {
            return Ok(None);
        }

        let mut buf = [0u8; 4];
        file.read_exact(&mut buf)?;
        file.read_exact(&mut buf)?;
        let entry_count = u32::from_be_bytes(buf);
        file.read_exact(&mut buf)?;

        let normalized_targets: Vec<String> = entry_names
            .iter()
            .map(|name| name.replace('\\', "/").to_lowercase())
            .collect();

        for _ in 0..entry_count {
            let mut tmp = [0u8; 4];
            file.read_exact(&mut tmp)?;
            let offset = u32::from_be_bytes(tmp) as u64;
            file.read_exact(&mut tmp)?;
            let size = u32::from_be_bytes(tmp) as usize;

            let mut name_bytes = Vec::with_capacity(64);
            loop {
                let mut b = [0u8; 1];
                file.read_exact(&mut b)?;
                if b[0] == 0 {
                    break;
                }
                name_bytes.push(b[0]);
            }

            let normalized_name = String::from_utf8_lossy(&name_bytes)
                .replace('\\', "/")
                .to_lowercase();

            if normalized_targets
                .iter()
                .any(|target| *target == normalized_name)
            {
                if size == 0 {
                    return Ok(None);
                }

                let current_pos = file.stream_position()?;
                file.seek(SeekFrom::Start(offset))?;
                let mut data = vec![0u8; size];
                file.read_exact(&mut data)?;
                file.seek(SeekFrom::Start(current_pos))?;
                return Ok(Some(data));
            }
        }

        Ok(None)
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
            let resolved_path = match kind {
                TextureKind::Diffuse => {
                    if texture.resolved_path.is_none() {
                        if let Some(path) = Self::resolve_texture_path(&texture.diffuse_path) {
                            texture.resolved_path = Some(path);
                        }
                    }
                    texture.resolved_path.clone()
                }
                TextureKind::Normal => {
                    if texture.resolved_normal_path.is_none() {
                        if let Some(normal) = texture.normal_path.clone() {
                            if let Some(path) = Self::resolve_texture_path(&normal) {
                                texture.resolved_normal_path = Some(path);
                            } else {
                                warn!("Terrain normal map '{}' not found", normal);
                            }
                        }
                    }
                    texture.resolved_normal_path.clone()
                }
                TextureKind::Height => {
                    if texture.resolved_height_path.is_none() {
                        if let Some(height) = texture.height_path.clone() {
                            if let Some(path) = Self::resolve_texture_path(&height) {
                                texture.resolved_height_path = Some(path);
                            } else {
                                warn!("Terrain height map '{}' not found", height);
                            }
                        }
                    }
                    texture.resolved_height_path.clone()
                }
            };

            if let Some(path) = resolved_path {
                match image::open(&path) {
                    Ok(image) => {
                        let rgba = image.to_rgba8();
                        let (texture, view) = create_gpu_texture_from_rgba(
                            device,
                            queue,
                            &rgba,
                            image.width(),
                            image.height(),
                            Some(&format!("Terrain Texture {}", path.display())),
                        );
                        return Ok((texture, view));
                    }
                    Err(err) => {
                        warn!(
                            "Failed to open terrain texture '{}': {}; using fallback",
                            path.display(),
                            err
                        );
                    }
                }
            } else {
                let source_path = match kind {
                    TextureKind::Diffuse => texture.diffuse_path.as_str(),
                    TextureKind::Normal => texture.normal_path.as_deref().unwrap_or(""),
                    TextureKind::Height => texture.height_path.as_deref().unwrap_or(""),
                };

                if !source_path.is_empty() {
                    if let Some(image) = Self::load_image_from_game_fs(source_path) {
                        let rgba = image.to_rgba8();
                        let (texture, view) = create_gpu_texture_from_rgba(
                            device,
                            queue,
                            &rgba,
                            image.width(),
                            image.height(),
                            Some(&format!("Terrain Texture {}", source_path)),
                        );
                        return Ok((texture, view));
                    }
                }
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

        let resolved = Self::resolve_texture_path(&texture.diffuse_path);
        let fallback_image = if resolved.is_none() {
            Self::load_image_from_game_fs(&texture.diffuse_path)
        } else {
            None
        };

        if let Some(path) = resolved.clone() {
            texture.resolved_path = Some(path.clone());
            match image::open(&path) {
                Ok(img) => {
                    texture.dimensions = Some(img.dimensions());
                }
                Err(err) => {
                    warn!(
                        "Failed to open terrain texture '{}': {}",
                        path.display(),
                        err
                    );
                    texture.dimensions = Some(DEFAULT_TEXTURE_DIMENSIONS);
                }
            }
        } else if let Some(img) = fallback_image {
            texture.dimensions = Some(img.dimensions());
        } else {
            warn!(
                "Terrain texture '{}' could not be resolved; using default dimensions",
                texture.diffuse_path
            );
            texture.dimensions = Some(DEFAULT_TEXTURE_DIMENSIONS);
        }

        if texture.normal_path.is_none() {
            if let Some(base) = texture.resolved_path.as_ref().or(resolved.as_ref()) {
                if let Some(candidate) = find_companion_map(base, &["_n.dds", "_normal.dds"]) {
                    texture.normal_path = Some(candidate.to_string_lossy().into_owned());
                    texture.resolved_normal_path = Some(candidate);
                }
            }
        }

        if texture.height_path.is_none() {
            if let Some(base) = texture.resolved_path.as_ref().or(resolved.as_ref()) {
                if let Some(candidate) = find_companion_map(base, &["_h.dds", "_height.dds"]) {
                    texture.height_path = Some(candidate.to_string_lossy().into_owned());
                    texture.resolved_height_path = Some(candidate);
                }
            }
        }

        if let Some(normal_path) = texture.normal_path.clone() {
            if texture.resolved_normal_path.is_none() {
                if let Some(resolved) = Self::resolve_texture_path(&normal_path) {
                    texture.resolved_normal_path = Some(resolved);
                } else {
                    warn!("Terrain normal map '{}' could not be resolved", normal_path);
                }
            }
        }

        if let Some(height_path) = texture.height_path.clone() {
            if texture.resolved_height_path.is_none() {
                if let Some(resolved) = Self::resolve_texture_path(&height_path) {
                    texture.resolved_height_path = Some(resolved);
                } else {
                    warn!("Terrain height map '{}' could not be resolved", height_path);
                }
            }
        }

        if let Some(height_resolved) = texture.resolved_height_path.clone() {
            match image::open(&height_resolved) {
                Ok(img) => {
                    let luma = img.to_luma8();
                    texture.height_image = Some(Arc::new(HeightImage {
                        width: luma.width(),
                        height: luma.height(),
                        data: luma.into_raw(),
                    }));
                }
                Err(err) => {
                    warn!(
                        "Failed to open terrain height map '{}': {}",
                        height_resolved.display(),
                        err
                    );
                }
            }
        } else if let Some(height_path) = texture.height_path.clone() {
            if let Some(img) = Self::load_image_from_game_fs(&height_path) {
                let luma = img.to_luma8();
                texture.height_image = Some(Arc::new(HeightImage {
                    width: luma.width(),
                    height: luma.height(),
                    data: luma.into_raw(),
                }));
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
            if self
                .textures
                .values()
                .any(|texture| texture.diffuse_path == *path)
            {
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

    /// Create texture atlas from multiple textures
    pub fn create_atlas(
        &mut self,
        texture_ids: &[TextureId],
        atlas_size: (u32, u32),
    ) -> TerrainResult<u32> {
        let atlas_id = self.next_atlas_id;
        let mut atlas = TextureAtlas::new(atlas_id, atlas_size.0, atlas_size.1);

        // Simple grid packing (could be improved with proper bin packing)
        let cols = (atlas_size.0 as f32 / 256.0).floor() as u32;
        let rows = (atlas_size.1 as f32 / 256.0).floor() as u32;

        for (i, &texture_id) in texture_ids.iter().enumerate() {
            if i >= (cols * rows) as usize {
                break; // Atlas full
            }

            let col = (i as u32) % cols;
            let row = (i as u32) / cols;

            let original_size = self
                .textures
                .get(&texture_id)
                .and_then(|texture| texture.dimensions)
                .unwrap_or(DEFAULT_TEXTURE_DIMENSIONS);

            let region = AtlasRegion {
                texture_id,
                uv_min: Vec2::new((col as f32) / (cols as f32), (row as f32) / (rows as f32)),
                uv_max: Vec2::new(
                    ((col + 1) as f32) / (cols as f32),
                    ((row + 1) as f32) / (rows as f32),
                ),
                original_size,
            };

            atlas.add_region(texture_id, region);
        }

        atlas.rebuild(&self.textures)?;
        self.atlases.insert(atlas_id, atlas);
        self.next_atlas_id += 1;
        self.stats.total_atlases += 1;

        Ok(atlas_id)
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
            let (gradient, height_strength) = self
                .textures
                .get(&rule.texture_id)
                .map(|texture| {
                    let gradient = texture
                        .height_image
                        .as_deref()
                        .map(|image| sample_height_gradient(image, tex_coords))
                        .unwrap_or(0.0);
                    (gradient, texture.material.height_strength.max(0.0))
                })
                .unwrap_or((0.0, 0.0));

            let base_weight = rule.calculate_weight(height, slope, gradient, height_strength);
            self.record_blend_debug(
                "rule_base",
                height,
                slope,
                tex_coords,
                rule.texture_id,
                Some(gradient),
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
                Some(gradient),
                weight,
            );

            contributions
                .entry(rule.texture_id)
                .and_modify(|existing| *existing += weight)
                .or_insert(weight);
        }

        self.stats.blend_operations = self.stats.blend_operations.wrapping_add(1);

        if contributions.is_empty() {
            return if let Some(rule) = texture_rules.first() {
                TextureWeights::single(rule.texture_id)
            } else {
                TextureWeights::single(0)
            };
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
            return TextureWeights::single(0);
        }

        let mut adjusted_weights = base_weights.clone();
        let mut adjusted = false;
        let slope = normal.dot(Vec3::Y).clamp(-1.0, 1.0).acos();

        for idx in 0..MAX_BLEND_WEIGHTS {
            if idx >= adjusted_weights.indices.len() {
                break;
            }

            let weight = adjusted_weights.weights[idx];
            if weight <= f32::EPSILON {
                continue;
            }

            let texture_id = adjusted_weights.indices[idx];
            if let Some((image, height_strength)) = self.height_image_for(texture_id) {
                let gradient_mag = sample_height_gradient(image, tex_coords);
                self.record_blend_debug(
                    "blend_input",
                    height,
                    slope,
                    tex_coords,
                    texture_id,
                    Some(gradient_mag),
                    weight,
                );
                if gradient_mag > f32::EPSILON {
                    let gradient_factor =
                        1.0 + gradient_mag * (0.35 + height_strength.clamp(0.0, 1.0));
                    adjusted_weights.weights[idx] *= gradient_factor;
                    self.record_blend_debug(
                        "blend_adjusted",
                        height,
                        slope,
                        tex_coords,
                        texture_id,
                        Some(gradient_mag),
                        adjusted_weights.weights[idx],
                    );
                    adjusted = true;
                }
            }
        }

        if adjusted {
            adjusted_weights.normalize();
            if self.blend_debug_path.is_some() {
                for (texture_id, weight) in adjusted_weights.iter_pairs() {
                    let gradient = self
                        .height_image_for(texture_id)
                        .map(|(image, _)| sample_height_gradient(image, tex_coords));
                    self.record_blend_debug(
                        "blend_final",
                        height,
                        slope,
                        tex_coords,
                        texture_id,
                        gradient,
                        weight,
                    );
                }
            }
        }

        adjusted_weights
    }

    /// Get texture by ID
    pub fn get_texture(&self, texture_id: TextureId) -> Option<&TerrainTexture> {
        self.textures.get(&texture_id)
    }

    /// Sample representative color for the terrain at a world position.
    pub fn sample_color_at(&self, _x: f32, _y: f32) -> TerrainResult<[f32; 3]> {
        if let Some(texture) = self.textures.values().next() {
            Ok(texture.material.albedo)
        } else {
            Ok([0.5, 0.5, 0.5])
        }
    }

    /// Determine terrain texture identifier at a world position.
    pub fn get_terrain_type_at(&self, _x: f32, _y: f32) -> TerrainResult<u32> {
        if let Some((&texture_id, _)) = self.textures.iter().next() {
            Ok(texture_id as u32)
        } else {
            Ok(0)
        }
    }

    /// Get mutable texture by ID
    pub fn get_texture_mut(&mut self, texture_id: TextureId) -> Option<&mut TerrainTexture> {
        self.textures.get_mut(&texture_id)
    }

    /// Get atlas by ID
    pub fn get_atlas(&self, atlas_id: u32) -> Option<&TextureAtlas> {
        self.atlases.get(&atlas_id)
    }

    /// Update texture system (reload dirty atlases, etc.)
    pub fn update(&mut self) -> TerrainResult<()> {
        let start_time = std::time::Instant::now();

        // Rebuild dirty atlases
        let dirty_atlases: Vec<u32> = self
            .atlases
            .values()
            .filter(|atlas| atlas.dirty)
            .map(|atlas| atlas.id)
            .collect();

        for atlas_id in dirty_atlases {
            if let Some(atlas) = self.atlases.get_mut(&atlas_id) {
                atlas.rebuild(&self.textures)?;
            }
        }

        self.stats.last_update_time = start_time.elapsed();
        Ok(())
    }

    fn height_image_for(&self, texture_id: TextureId) -> Option<(&HeightImage, f32)> {
        self.textures.get(&texture_id).and_then(|texture| {
            texture
                .height_image
                .as_deref()
                .map(|image| (image, texture.material.height_strength.max(0.0)))
        })
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

    /// Clear all textures and atlases
    pub fn clear(&mut self) {
        self.textures.clear();
        self.atlases.clear();
        self.next_texture_id = 1;
        self.next_atlas_id = 1;
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
        gradient: f32,
        height_strength: f32,
    ) -> f32 {
        let height_factor = if height >= self.height_min && height <= self.height_max {
            1.0 - ((height - (self.height_min + self.height_max) / 2.0).abs()
                / ((self.height_max - self.height_min) / 2.0))
                .min(1.0)
        } else {
            0.0
        };

        let slope_factor = if slope >= self.slope_min && slope <= self.slope_max {
            1.0 - ((slope - (self.slope_min + self.slope_max) / 2.0).abs()
                / ((self.slope_max - self.slope_min) / 2.0))
                .min(1.0)
        } else {
            0.0
        };

        if height_factor <= 0.0 || slope_factor <= 0.0 {
            return 0.0;
        }

        let scaled_gradient = (gradient * 8.0).min(1.0);
        if scaled_gradient <= f32::EPSILON {
            return height_factor * slope_factor;
        }

        let preferred = if self.preferred_gradient >= 0.0 {
            self.preferred_gradient.clamp(0.0, 1.0)
        } else {
            (height_strength * 8.0).clamp(0.0, 1.0)
        };
        let material_preference = (height_strength * 4.0).clamp(0.0, 1.0);
        let combined_preference = ((preferred * 0.7) + (material_preference * 0.3)).clamp(0.0, 1.0);
        let tolerance = self.gradient_tolerance.max(0.05).min(1.0);

        let gradient_delta = (scaled_gradient - combined_preference) / tolerance;
        let gradient_factor = (-gradient_delta * gradient_delta).exp().clamp(0.0, 1.0);

        (height_factor * slope_factor * gradient_factor.max(0.0)).max(0.0)
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
    use std::collections::HashMap;
    use std::sync::Arc;

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
    fn test_atlas_creation() {
        let mut manager = TextureManager::new();
        let texture_ids = vec![
            manager.register_texture(TerrainTexture::new(
                0,
                "tex1".to_string(),
                "tex1.jpg".to_string(),
            )),
            manager.register_texture(TerrainTexture::new(
                0,
                "tex2".to_string(),
                "tex2.jpg".to_string(),
            )),
        ];

        let atlas_result = manager.create_atlas(&texture_ids, (512, 512));
        assert!(atlas_result.is_ok());

        let atlas_id = atlas_result.unwrap();
        assert!(manager.get_atlas(atlas_id).is_some());
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
        assert!(rock_weight_steep > rock_weight_shallow);

        let dirt_weight_steep = dirt_rule.calculate_weight(height, slope, steep_gradient, 0.5);
        let dirt_weight_shallow = dirt_rule.calculate_weight(height, slope, shallow_gradient, 0.5);
        assert!(dirt_weight_shallow > dirt_weight_steep);
    }

    #[test]
    fn test_generate_texture_weights_gradient_bias() {
        let mut manager = TextureManager::new();
        let tex_rock = TerrainTexture::new(0, "rock".to_string(), "rock.png".to_string());
        let tex_dirt = TerrainTexture::new(0, "dirt".to_string(), "dirt.png".to_string());

        let rock_id = manager.register_texture(tex_rock);
        let dirt_id = manager.register_texture(tex_dirt);

        if let Some(texture) = manager.get_texture_mut(rock_id) {
            texture.height_image = Some(Arc::new(HeightImage {
                width: 2,
                height: 2,
                data: vec![0, 0, 255, 255],
            }));
            texture.material.height_strength = 1.0;
        }
        if let Some(texture) = manager.get_texture_mut(dirt_id) {
            texture.height_image = Some(Arc::new(HeightImage {
                width: 2,
                height: 2,
                data: vec![0, 0, 255, 255],
            }));
            texture.material.height_strength = 0.2;
        }

        let rules = vec![
            TextureRule {
                texture_id: rock_id,
                height_min: 0.0,
                height_max: 200.0,
                slope_min: 0.0,
                slope_max: std::f32::consts::PI,
                priority: 0,
                preferred_gradient: 0.75,
                gradient_tolerance: 0.25,
            },
            TextureRule {
                texture_id: dirt_id,
                height_min: 0.0,
                height_max: 200.0,
                slope_min: 0.0,
                slope_max: std::f32::consts::PI,
                priority: 0,
                preferred_gradient: 0.05,
                gradient_tolerance: 0.25,
            },
        ];

        let weights = manager.generate_texture_weights(50.0, 0.2, [0.5, 0.5], &rules);
        let weight_map: HashMap<TextureId, f32> = weights.iter_pairs().collect();

        let rock_weight = weight_map.get(&rock_id).copied().unwrap_or(0.0);
        let dirt_weight = weight_map.get(&dirt_id).copied().unwrap_or(0.0);

        assert!(rock_weight > dirt_weight);
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

fn sample_height_value_from_image(image: &HeightImage, uv: [f32; 2]) -> f32 {
    if image.width == 0 || image.height == 0 {
        return 0.0;
    }

    let u = wrap_repeat(uv[0]);
    let v = wrap_repeat(uv[1]);
    let x = u * (image.width.saturating_sub(1) as f32);
    let y = v * (image.height.saturating_sub(1) as f32);

    let x0 = x.floor() as usize;
    let y0 = y.floor() as usize;
    let x1 = (x0 + 1).min(image.width as usize - 1);
    let y1 = (y0 + 1).min(image.height as usize - 1);

    let tx = x - x0 as f32;
    let ty = y - y0 as f32;

    let idx = |ix: usize, iy: usize| -> f32 {
        let offset = iy * image.width as usize + ix;
        image
            .data
            .get(offset)
            .map(|v| *v as f32 / 255.0)
            .unwrap_or(0.0)
    };

    let h00 = idx(x0, y0);
    let h10 = idx(x1, y0);
    let h01 = idx(x0, y1);
    let h11 = idx(x1, y1);

    let hx0 = h00 * (1.0 - tx) + h10 * tx;
    let hx1 = h01 * (1.0 - tx) + h11 * tx;

    hx0 * (1.0 - ty) + hx1 * ty
}

fn sample_height_gradient(image: &HeightImage, uv: [f32; 2]) -> f32 {
    if image.width <= 1 || image.height <= 1 {
        return 0.0;
    }

    let delta_u = 1.0f32 / image.width as f32;
    let delta_v = 1.0f32 / image.height as f32;

    let sample_left = sample_height_value_from_image(image, [uv[0] - delta_u, uv[1]]);
    let sample_right = sample_height_value_from_image(image, [uv[0] + delta_u, uv[1]]);
    let sample_down = sample_height_value_from_image(image, [uv[0], uv[1] - delta_v]);
    let sample_up = sample_height_value_from_image(image, [uv[0], uv[1] + delta_v]);

    let gradient_x = 0.5 * (sample_right - sample_left);
    let gradient_y = 0.5 * (sample_up - sample_down);

    (gradient_x * gradient_x + gradient_y * gradient_y).sqrt()
}

fn wrap_repeat(value: f32) -> f32 {
    let mut v = value % 1.0;
    if v < 0.0 {
        v += 1.0;
    }
    v
}

fn find_companion_map(base: &Path, suffixes: &[&str]) -> Option<PathBuf> {
    let parent = base.parent()?;
    let stem = base
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or_default()
        .to_string();

    for suffix in suffixes {
        let candidate = parent.join(format!("{}{}", stem, suffix));
        if candidate.exists() {
            return Some(candidate);
        }
        let candidate_upper = parent.join(format!("{}{}", stem, suffix.to_uppercase()));
        if candidate_upper.exists() {
            return Some(candidate_upper);
        }
    }

    None
}
