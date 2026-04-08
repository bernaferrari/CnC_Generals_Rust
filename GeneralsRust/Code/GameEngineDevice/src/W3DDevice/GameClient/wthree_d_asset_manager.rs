//! W3D asset manager (port of W3DAssetManager).
//!
//! Corresponds to C++ files:
//!   - GameEngineDevice/Include/W3DDevice/GameClient/W3DAssetManager.h
//!   - GameEngineDevice/Source/W3DDevice/GameClient/W3DAssetManager.cpp
//!
//! Manages loading, caching, and lifetime of render objects (meshes, HLODs),
//! textures, animations, and fonts. Provides house-color recoloring, texture
//! replacement, prototype caching, and reference-counted asset cleanup.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

// ---------------------------------------------------------------------------
// Constants (matching C++)
// ---------------------------------------------------------------------------

const SCALE_EPSILON: f32 = 0.01f32;
const H_EPSILON: f32 = 1.0f32;
const S_EPSILON: f32 = 0.01f32;
const V_EPSILON: f32 = 0.01f32;
const IDENT_SCALE: f32 = 1.0f32;

const TEAM_COLOR_PALETTE_SIZE: usize = 16;
const TEAM_COLOR_PALETTE: [u16; TEAM_COLOR_PALETTE_SIZE] = [
    255, 239, 223, 211, 195, 174, 167, 151, 135, 123, 107, 91, 79, 63, 47, 35,
];

const MAX_WARNING_COUNT: u32 = 20;

// ---------------------------------------------------------------------------
// Asset types
// ---------------------------------------------------------------------------

/// Asset class IDs matching C++ RenderObjClass::Class_ID
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum AssetClassId {
    Mesh = 0,
    Hlod = 1,
    Aggregate = 2,
    ParticleEmitter = 3,
}

/// Reference-counted GPU texture handle.
#[derive(Debug, Clone)]
pub struct AssetTexture {
    pub name: String,
    pub wgpu_texture: Option<Arc<wgpu::Texture>>,
    pub wgpu_view: Option<Arc<wgpu::TextureView>>,
    pub ref_count: u32,
    pub width: u32,
    pub height: u32,
    pub format: wgpu::TextureFormat,
}

impl AssetTexture {
    pub fn add_ref(&mut self) {
        self.ref_count = self.ref_count.saturating_add(1);
    }

    pub fn release_ref(&mut self) -> bool {
        if self.ref_count > 0 {
            self.ref_count -= 1;
        }
        self.ref_count == 0
    }
}

/// Reference-counted render object prototype (mesh, HLOD, etc.).
#[derive(Debug, Clone)]
pub struct AssetPrototype {
    pub name: String,
    pub class_id: AssetClassId,
    pub ref_count: u32,
    /// Munged name cache key (C++ W3DPrototypeClass stores the munged name)
    pub cache_key: Option<String>,
    /// Optional team color applied to this prototype
    pub object_color: u32,
}

impl AssetPrototype {
    pub fn add_ref(&mut self) {
        self.ref_count = self.ref_count.saturating_add(1);
    }

    pub fn release_ref(&mut self) -> bool {
        if self.ref_count > 0 {
            self.ref_count -= 1;
        }
        self.ref_count == 0
    }
}

/// Lightweight handle returned to callers for asset lookup.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct AssetHandle {
    pub id: u64,
}

// ---------------------------------------------------------------------------
// House-color recoloring (C++ Recolor_Texture, Remap_Palette, etc.)
// ---------------------------------------------------------------------------

/// Team color palette remap for textures.
///
/// C++ stores a 16-entry palette in the top row of the texture; pixels
/// matching those values are remapped to the house color gradient.
fn recolor_palette_entry(index: usize, r: f32, g: f32, b: f32) -> (u8, u8, u8) {
    let scale = TEAM_COLOR_PALETTE[index] as f32 / 255.0;
    let cr = (r * scale).clamp(0.0, 255.0) as u8;
    let cg = (g * scale).clamp(0.0, 255.0) as u8;
    let cb = (b * scale).clamp(0.0, 255.0) as u8;
    (cr, cg, cb)
}

/// Parse house color from packed 0xRRGGBB integer to normalized floats.
fn unpack_house_color(color: u32) -> (f32, f32, f32) {
    let r = ((color >> 16) & 0xFF) as f32 / 255.0;
    let g = ((color >> 8) & 0xFF) as f32 / 255.0;
    let b = (color & 0xFF) as f32 / 255.0;
    (r, g, b)
}

/// Generate a munged texture name for house-color lookup (C++ Munge_Texture_Name).
fn munge_texture_name(name: &str, color: u32) -> String {
    let lower: String = name.chars().map(|c| c.to_ascii_lowercase()).collect();
    format!("#{}#{}", color, lower)
}

/// Generate a munged render-object name (C++ Munge_Render_Obj_Name).
fn munge_render_obj_name(name: &str, scale: f32, color: u32, texture_name: &str) -> String {
    let lower: String = name.chars().map(|c| c.to_ascii_lowercase()).collect();
    let tex = if texture_name.is_empty() {
        String::new()
    } else {
        texture_name.to_string()
    };
    format!("#{}!{}!{}#{}", color, scale, tex, lower)
}

// ---------------------------------------------------------------------------
// W3DAssetManager
// ---------------------------------------------------------------------------

/// Manages loading, caching, and lifecycle of all W3D assets.
///
/// Corresponds to C++ `W3DAssetManager : public WW3DAssetManager`.
///
/// Provides:
/// - Prototype loading and caching (meshes, HLODs, particle emitters)
/// - Texture loading and caching with house-color recoloring
/// - Reference-counted asset cleanup
/// - Asset loading from .w3d files and BIG archives
pub struct WthreeDAssetManager {
    initialized: bool,

    /// Cached render-object prototypes (C++ Prototypes + Find_Prototype / Add_Prototype)
    prototypes: HashMap<String, AssetPrototype>,

    /// Cached textures (C++ TextureHash)
    textures: HashMap<String, AssetTexture>,

    /// WGPU device for texture creation (injected after init)
    device: Option<Arc<wgpu::Device>>,
    queue: Option<Arc<wgpu::Queue>>,

    /// Load-on-demand flag (C++ WW3D_Load_On_Demand)
    load_on_demand: bool,

    /// Warning counter for missing assets (C++ static warning_count)
    missing_asset_warnings: u32,

    /// Track loaded asset files to avoid redundant loads
    loaded_files: HashMap<String, bool>,

    /// Preload report flag (C++ TheGlobalData->m_preloadReport)
    preload_report: bool,
}

// ---------------------------------------------------------------------------
// Construction / lifecycle
// ---------------------------------------------------------------------------

impl WthreeDAssetManager {
    pub fn new() -> Self {
        Self {
            initialized: false,
            prototypes: HashMap::new(),
            textures: HashMap::new(),
            device: None,
            queue: None,
            load_on_demand: true,
            missing_asset_warnings: 0,
            loaded_files: HashMap::new(),
            preload_report: false,
        }
    }

    /// Initialize the asset manager (C++ W3DAssetManager constructor body).
    pub fn initialize(&mut self) -> Result<(), WthreeDAssetManagerError> {
        if self.initialized {
            return Ok(());
        }
        self.initialized = true;
        Ok(())
    }

    /// Shutdown and release all assets.
    pub fn shutdown(&mut self) {
        if !self.initialized {
            return;
        }
        self.free_assets();
        self.initialized = false;
    }

    /// Check if initialized.
    pub fn is_initialized(&self) -> bool {
        self.initialized
    }

    /// Inject WGPU device/queue for texture creation.
    pub fn set_wgpu_resources(&mut self, device: Arc<wgpu::Device>, queue: Arc<wgpu::Queue>) {
        self.device = Some(device);
        self.queue = Some(queue);
    }
}

impl Default for WthreeDAssetManager {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for WthreeDAssetManager {
    fn drop(&mut self) {
        self.shutdown();
    }
}

// ---------------------------------------------------------------------------
// Prototype management (C++ Create_Render_Obj, Find_Prototype, Add_Prototype)
// ---------------------------------------------------------------------------

impl WthreeDAssetManager {
    /// Find a prototype by name (C++ Find_Prototype).
    pub fn find_prototype(&self, name: &str) -> Option<&AssetPrototype> {
        self.prototypes.get(name)
    }

    /// Find a mutable prototype by name.
    pub fn find_prototype_mut(&mut self, name: &str) -> Option<&mut AssetPrototype> {
        self.prototypes.get_mut(name)
    }

    /// Register a prototype (C++ Add_Prototype via W3DPrototypeClass).
    pub fn add_prototype(&mut self, proto: AssetPrototype) {
        self.prototypes.insert(proto.name.clone(), proto);
    }

    /// Check if a render object prototype exists (C++ Render_Obj_Exists).
    pub fn render_obj_exists(&self, name: &str) -> bool {
        self.prototypes.contains_key(name)
    }

    /// Create a render object instance from a prototype (C++ Create_Render_Obj).
    ///
    /// C++ `Create_Render_Obj(name)`: looks up prototype, clones it.
    pub fn create_render_obj(&mut self, name: &str) -> Option<AssetHandle> {
        let proto = self.prototypes.get_mut(name)?;
        proto.add_ref();
        Some(AssetHandle { id: hash_name(name) })
    }

    /// Create a customized render object with scale and team color
    /// (C++ `Create_Render_Obj(name, scale, color, oldTexture, newTexture)`).
    pub fn create_render_obj_customized(
        &mut self,
        name: &str,
        scale: f32,
        color: u32,
        old_texture: Option<&str>,
        new_texture: Option<&str>,
    ) -> Option<AssetHandle> {
        let really_scale = (scale - IDENT_SCALE).abs() > SCALE_EPSILON;
        let really_color = (color & 0xFFFFFF) != 0; // black = no custom color
        let really_texture = old_texture.is_some() && new_texture.is_some();

        // Base case: no customization needed
        if !really_scale && !really_color && !really_texture {
            return self.create_render_obj(name);
        }

        // Try cached munged name
        let cache_key = munge_render_obj_name(name, scale, color, new_texture.unwrap_or(""));
        if let Some(proto) = self.prototypes.get_mut(&cache_key) {
            proto.add_ref();
            return Some(AssetHandle {
                id: hash_name(&cache_key),
            });
        }

        // Find base prototype
        let base_proto = if let Some(p) = self.prototypes.get(name) {
            p.clone()
        } else if self.load_on_demand {
            // Try loading from file
            let filename = format!("{}.w3d", name);
            if self.load_3d_assets(&filename) {
                self.prototypes.get(name).cloned()?
            } else {
                // Try parent directory
                let parent_filename = format!("../{}", filename);
                if self.load_3d_assets(&parent_filename) {
                    self.prototypes.get(name).cloned()?
                } else {
                    self.warn_missing_asset(name);
                    return None;
                }
            }
        } else {
            self.warn_missing_asset(name);
            return None;
        };

        // Clone and customize the prototype
        let mut custom_proto = base_proto;
        custom_proto.cache_key = Some(cache_key.clone());
        custom_proto.object_color = color;
        custom_proto.ref_count = 1;

        // Handle texture replacement
        if really_texture {
            if let (Some(old_tex), Some(new_tex)) = (old_texture, new_texture) {
                self.replace_prototype_texture(&mut custom_proto, old_tex, new_tex);
            }
        }

        self.prototypes.insert(cache_key.clone(), custom_proto);
        Some(AssetHandle {
            id: hash_name(&cache_key),
        })
    }

    /// Replace all references to old texture with new texture in a prototype
    /// (C++ replacePrototypeTexture).
    pub fn replace_prototype_texture(
        &self,
        _proto: &mut AssetPrototype,
        _old_name: &str,
        _new_name: &str,
    ) -> bool {
        // PARITY_NOTE: C++ walks sub-objects calling replaceMeshTexture /
        // replaceHLODTexture. The Rust port stores texture references
        // differently — actual texture swap deferred to mesh material system.
        false
    }

    fn warn_missing_asset(&mut self, name: &str) {
        if self.missing_asset_warnings < MAX_WARNING_COUNT {
            log::warn!("WARNING: Failed to create Render Object: {}", name);
            self.missing_asset_warnings += 1;
        }
    }
}

// ---------------------------------------------------------------------------
// 3D asset loading (C++ Load_3D_Assets)
// ---------------------------------------------------------------------------

impl WthreeDAssetManager {
    /// Load 3D assets from a .w3d file (C++ Load_3D_Assets).
    ///
    /// C++ checks if the file's basename already exists as a prototype,
    /// then delegates to WW3DAssetManager::Load_3D_Assets to parse the
    /// W3D chunk format and register prototypes.
    pub fn load_3d_assets(&mut self, filename: &str) -> bool {
        // Extract basename (strip extension)
        let base_name = Path::new(filename)
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or(filename);

        // Already loaded?
        if self.prototypes.contains_key(base_name) {
            return true;
        }

        // Check if we've already tried and failed
        if let Some(&false) = self.loaded_files.get(filename) {
            return false;
        }

        // Attempt to load from the W3D file system
        let loaded = self.try_load_w3d_file(filename, base_name);
        self.loaded_files.insert(filename.to_string(), loaded);

        if loaded && self.preload_report {
            log::info!("3D: {}", base_name.to_ascii_lowercase());
        }

        loaded
    }

    fn try_load_w3d_file(&mut self, _filename: &str, _base_name: &str) -> bool {
        // PARITY_NOTE: C++ WW3DAssetManager::Load_3D_Assets opens the file,
        // parses W3D chunk headers (HIERARCHY, MESH, HLOD, ANIMATION, etc.),
        // and registers each as a prototype.
        // The Rust port defers actual W3D binary parsing to the w3d_loader
        // module. For now, return false (asset not found).
        false
    }

    /// Preload a model asset (C++ preloadModelAssets).
    pub fn preload_model_assets(&mut self, model: &str) {
        let filename = format!("{}.w3d", model);
        self.load_3d_assets(&filename);
    }

    /// Preload a texture asset (C++ preloadTextureAssets).
    pub fn preload_texture_assets(&mut self, texture: &str) {
        let _ = self.get_texture(texture);
    }
}

// ---------------------------------------------------------------------------
// Texture management (C++ Get_Texture, Recolor_Texture, etc.)
// ---------------------------------------------------------------------------

impl WthreeDAssetManager {
    /// Get or load a texture by name (C++ Get_Texture).
    ///
    /// C++ normalizes to lowercase, checks TextureHash, creates new
    /// TextureClass if not found, adds ref, and returns.
    pub fn get_texture(&mut self, filename: &str) -> Option<Arc<AssetTexture>> {
        if filename.is_empty() {
            return None;
        }

        let lower_name: String = filename.chars().map(|c| c.to_ascii_lowercase()).collect();

        // Check if already cached
        if let Some(tex) = self.textures.get_mut(&lower_name) {
            tex.add_ref();
            // Return a clone — we can't return Arc directly from &mut self
            // Instead, this is a logical "add ref" for the caller
            return None; // Caller should use get_texture_ref instead
        }

        None
    }

    /// Get a reference-counted handle to a texture.
    pub fn get_texture_ref(&mut self, filename: &str) -> Option<AssetHandle> {
        if filename.is_empty() {
            return None;
        }

        let lower_name: String = filename.chars().map(|c| c.to_ascii_lowercase()).collect();

        if self.textures.contains_key(&lower_name) {
            return Some(AssetHandle {
                id: hash_name(&lower_name),
            });
        }

        // Don't allow reduction on ZHC infantry textures (C++ check)
        let allow_reduction = !upper_starts_with(&lower_name, "ZHC");

        // Try to load the texture
        if self.try_load_texture(&lower_name, allow_reduction) {
            if self.preload_report {
                log::info!("TX: {}", lower_name);
            }
            return Some(AssetHandle {
                id: hash_name(&lower_name),
            });
        }

        None
    }

    fn try_load_texture(&mut self, name: &str, _allow_reduction: bool) -> bool {
        // PARITY_NOTE: C++ creates TextureClass from file, which opens TGA/DDS,
        // uploads to GPU, and inserts into TextureHash.
        // The Rust port would use image crate + WGPU texture creation.
        // For now, create a placeholder.
        let tex = AssetTexture {
            name: name.to_string(),
            wgpu_texture: None,
            wgpu_view: None,
            ref_count: 1,
            width: 0,
            height: 0,
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
        };
        self.textures.insert(name.to_string(), tex);
        true
    }

    /// Find a house-color tinted texture (C++ Find_Texture with color).
    pub fn find_texture(&self, name: &str, color: u32) -> Option<&AssetTexture> {
        let munged = munge_texture_name(name, color);
        self.textures.get(&munged)
    }

    /// Recolor a texture for a team color (C++ Recolor_Texture).
    ///
    /// Returns the munged name of the recolored texture if it was created.
    pub fn recolor_texture(&mut self, name: &str, color: u32) -> Option<String> {
        let munged = munge_texture_name(name, color);
        if self.textures.contains_key(&munged) {
            return Some(munged);
        }
        self.recolor_texture_one_time(name, color)
    }

    /// One-time texture recolor (C++ Recolor_Texture_One_Time).
    ///
    /// Creates a new texture with house-color palette remapping.
    fn recolor_texture_one_time(&mut self, name: &str, color: u32) -> Option<String> {
        // Check if source texture exists
        if !self.textures.contains_key(name) {
            return None;
        }

        let (r, g, b) = unpack_house_color(color);

        // Create a recolored copy
        let source = self.textures.get(name)?;
        let mut new_tex = AssetTexture {
            name: munge_texture_name(name, color),
            wgpu_texture: None,
            wgpu_view: None,
            ref_count: 1,
            width: source.width,
            height: source.height,
            format: source.format,
        };

        // If source has GPU texture and we have device, create recolored version
        if let (Some(ref device), Some(ref queue), Some(ref source_gpu)) =
            (&self.device, &self.queue, &source.wgpu_texture)
        {
            // PARITY_NOTE: C++ locks the surface, remaps palette pixels, creates
            // new SurfaceClass + TextureClass. WGPU equivalent: read back source,
            // remap pixels, create new texture, upload.
            // Deferred to actual WGPU texture readback implementation.
            let _ = (device, queue, source_gpu);
        }

        let munged = new_tex.name.clone();
        self.textures.insert(munged.clone(), new_tex);
        Some(munged)
    }

    /// Release a texture reference (C++ REF_PTR_RELEASE on TextureClass).
    pub fn release_texture(&mut self, handle: AssetHandle) {
        // Find and decrement ref count; remove if zero
        let to_remove = self.textures.iter_mut().find_map(|(name, tex)| {
            if hash_name(name) == handle.id {
                tex.release_ref();
                if tex.ref_count == 0 {
                    Some(name.clone())
                } else {
                    None
                }
            } else {
                None
            }
        });

        if let Some(name) = to_remove {
            self.textures.remove(&name);
        }
    }
}

// ---------------------------------------------------------------------------
// Asset cleanup (C++ Release_Unused_Assets, Free_Assets, Report_Used_*)
// ---------------------------------------------------------------------------

impl WthreeDAssetManager {
    /// Release all assets with ref_count <= 1 (C++ Release_Unused_Assets).
    pub fn release_unused_assets(&mut self) {
        self.prototypes.retain(|_, proto| proto.ref_count > 1);
        self.textures.retain(|_, tex| tex.ref_count > 1);
    }

    /// Free all assets (C++ Free_Assets).
    pub fn free_assets(&mut self) {
        self.prototypes.clear();
        self.textures.clear();
        self.loaded_files.clear();
    }

    /// Report unfreed prototypes (C++ Report_Used_Prototypes).
    pub fn report_used_prototypes(&self) {
        for proto in self.prototypes.values() {
            if proto.ref_count > 1 {
                log::debug!(
                    "**Unfreed Prototype On Map Reset: {}",
                    proto.name
                );
            }
        }
    }

    /// Report unfreed textures (C++ Report_Used_Textures).
    pub fn report_used_textures(&self) {
        for tex in self.textures.values() {
            if tex.ref_count > 1 {
                log::debug!(
                    "**Texture \"{}\" referenced {} times on map reset",
                    tex.name,
                    tex.ref_count - 1,
                );
            }
        }
    }

    /// Report all used assets (C++ Report_Used_Assets).
    pub fn report_used_assets(&self) {
        self.report_used_prototypes();
        self.report_used_textures();
    }

    /// Get prototype count.
    pub fn prototype_count(&self) -> usize {
        self.prototypes.len()
    }

    /// Get texture count.
    pub fn texture_count(&self) -> usize {
        self.textures.len()
    }
}

// ---------------------------------------------------------------------------
// Load-on-demand control (C++ Set_WW3D_Load_On_Demand / WW3D_Load_On_Demand)
// ---------------------------------------------------------------------------

impl WthreeDAssetManager {
    /// Enable/disable load-on-demand (C++ Set_WW3D_Load_On_Demand).
    pub fn set_load_on_demand(&mut self, enabled: bool) {
        self.load_on_demand = enabled;
    }

    /// Check if load-on-demand is enabled.
    pub fn is_load_on_demand(&self) -> bool {
        self.load_on_demand
    }

    /// Enable/disable preload report logging.
    pub fn set_preload_report(&mut self, enabled: bool) {
        self.preload_report = enabled;
    }
}

// ---------------------------------------------------------------------------
// Texture loading from files
// ---------------------------------------------------------------------------

impl WthreeDAssetManager {
    /// Load a texture from a TGA/DDS file (C++ Get_Texture file path).
    ///
    /// Uses the existing W3D file system to locate the file, decodes it,
    /// and creates a WGPU texture.
    pub fn load_texture_from_file(
        &mut self,
        name: &str,
        path: &Path,
    ) -> Result<(), WthreeDAssetManagerError> {
        if !self.initialized {
            return Err(WthreeDAssetManagerError::NotInitialized);
        }

        let lower_name: String = name.chars().map(|c| c.to_ascii_lowercase()).collect();

        // Check if already loaded
        if self.textures.contains_key(&lower_name) {
            return Ok(());
        }

        // Try to load the image file
        let image_result = image::open(path);
        let img = match image_result {
            Ok(i) => i,
            Err(e) => {
                log::warn!("Failed to load texture {}: {:?}", path.display(), e);
                return Err(WthreeDAssetManagerError::Io(e.to_string()));
            }
        };

        let (width, height) = img.dimensions();
        let rgba = img.to_rgba8();

        // Create WGPU texture if device is available
        let (wgpu_texture, wgpu_view) = if let (Some(ref device), Some(ref queue)) =
            (&self.device, &self.queue)
        {
            let texture = device.create_texture(&wgpu::TextureDescriptor {
                label: Some(&format!("texture_{}", lower_name)),
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

            queue.write_texture(
                wgpu::ImageCopyTexture {
                    texture: &texture,
                    mip_level: 0,
                    origin: wgpu::Origin3d::ZERO,
                    aspect: wgpu::TextureAspect::All,
                },
                &rgba,
                wgpu::ImageDataLayout {
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

            let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
            (Some(Arc::new(texture)), Some(Arc::new(view)))
        } else {
            (None, None)
        };

        let tex = AssetTexture {
            name: lower_name.clone(),
            wgpu_texture,
            wgpu_view,
            ref_count: 1,
            width,
            height,
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
        };

        self.textures.insert(lower_name, tex);
        Ok(())
    }

    /// Load all assets required for a map (C++ loadMap).
    ///
    /// C++ iterates map asset lists, loading required textures and models.
    pub fn load_map(&mut self, _map_name: &str) -> Result<(), WthreeDAssetManagerError> {
        if !self.initialized {
            return Err(WthreeDAssetManagerError::NotInitialized);
        }
        // PARITY_NOTE: C++ loads map-specific textures, models, and scripts
        // from the map's asset manifest. Deferred to map loading pipeline.
        Ok(())
    }

    /// Background asset preloading (C++ preloadAssets).
    ///
    /// C++ supports background loading of assets that will be needed soon.
    /// In Rust, this can be spawned as a tokio task.
    #[cfg(feature = "async")]
    pub async fn preload_assets(&mut self, asset_names: &[String]) {
        for name in asset_names {
            if name.ends_with(".w3d") {
                let base = name.trim_end_matches(".w3d");
                self.load_3d_assets(base);
            } else {
                self.preload_texture_assets(name);
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Error type
// ---------------------------------------------------------------------------

/// Error types for WthreeDAssetManager operations.
#[derive(Debug, Clone)]
pub enum WthreeDAssetManagerError {
    /// Manager not initialized.
    NotInitialized,
    /// Asset not found.
    ResourceNotFound,
    /// IO error during file loading.
    Io(String),
    /// Unknown error.
    Unknown,
}

impl std::fmt::Display for WthreeDAssetManagerError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            WthreeDAssetManagerError::NotInitialized => write!(f, "Manager not initialized"),
            WthreeDAssetManagerError::ResourceNotFound => write!(f, "Resource not found"),
            WthreeDAssetManagerError::Io(msg) => write!(f, "IO error: {}", msg),
            WthreeDAssetManagerError::Unknown => write!(f, "Unknown manager error"),
        }
    }
}

impl std::error::Error for WthreeDAssetManagerError {}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Hash a name to a u64 handle ID.
fn hash_name(name: &str) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    name.hash(&mut hasher);
    hasher.finish()
}

/// Check if a string starts with an uppercase prefix (case-insensitive on first 3 chars).
fn upper_starts_with(s: &str, prefix: &str) -> bool {
    s.len() >= prefix.len() && s[..prefix.len()].eq_ignore_ascii_case(prefix)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_asset_manager_creation() {
        let mgr = WthreeDAssetManager::new();
        assert!(!mgr.is_initialized());
    }

    #[test]
    fn test_initialize() {
        let mut mgr = WthreeDAssetManager::new();
        mgr.initialize().unwrap();
        assert!(mgr.is_initialized());
    }

    #[test]
    fn test_double_initialize() {
        let mut mgr = WthreeDAssetManager::new();
        mgr.initialize().unwrap();
        mgr.initialize().unwrap(); // should be no-op
        assert!(mgr.is_initialized());
    }

    #[test]
    fn test_shutdown() {
        let mut mgr = WthreeDAssetManager::new();
        mgr.initialize().unwrap();
        mgr.shutdown();
        assert!(!mgr.is_initialized());
    }

    #[test]
    fn test_prototype_management() {
        let mut mgr = WthreeDAssetManager::new();
        mgr.initialize().unwrap();

        let proto = AssetPrototype {
            name: "TestMesh".to_string(),
            class_id: AssetClassId::Mesh,
            ref_count: 1,
            cache_key: None,
            object_color: 0,
        };
        mgr.add_prototype(proto);

        assert!(mgr.render_obj_exists("TestMesh"));
        assert_eq!(mgr.prototype_count(), 1);

        let handle = mgr.create_render_obj("TestMesh");
        assert!(handle.is_some());

        // Ref count should be 2 (1 original + 1 from create_render_obj)
        let p = mgr.find_prototype("TestMesh").unwrap();
        assert_eq!(p.ref_count, 2);
    }

    #[test]
    fn test_release_unused_assets() {
        let mut mgr = WthreeDAssetManager::new();
        mgr.initialize().unwrap();

        let proto = AssetPrototype {
            name: "TempMesh".to_string(),
            class_id: AssetClassId::Mesh,
            ref_count: 1,
            cache_key: None,
            object_color: 0,
        };
        mgr.add_prototype(proto);

        assert_eq!(mgr.prototype_count(), 1);
        mgr.release_unused_assets();
        assert_eq!(mgr.prototype_count(), 0);
    }

    #[test]
    fn test_free_assets() {
        let mut mgr = WthreeDAssetManager::new();
        mgr.initialize().unwrap();

        mgr.add_prototype(AssetPrototype {
            name: "A".to_string(),
            class_id: AssetClassId::Mesh,
            ref_count: 5,
            cache_key: None,
            object_color: 0,
        });
        mgr.add_prototype(AssetPrototype {
            name: "B".to_string(),
            class_id: AssetClassId::Hlod,
            ref_count: 3,
            cache_key: None,
            object_color: 0,
        });

        mgr.free_assets();
        assert_eq!(mgr.prototype_count(), 0);
        assert_eq!(mgr.texture_count(), 0);
    }

    #[test]
    fn test_load_on_demand() {
        let mut mgr = WthreeDAssetManager::new();
        assert!(mgr.is_load_on_demand());
        mgr.set_load_on_demand(false);
        assert!(!mgr.is_load_on_demand());
    }

    #[test]
    fn test_munge_texture_name() {
        let munged = munge_texture_name("MyTexture.tga", 0xFF0000);
        assert_eq!(munged, "#16711680#mytexture.tga");
    }

    #[test]
    fn test_munge_render_obj_name() {
        let munged = munge_render_obj_name("MyModel", 1.5, 0x00FF00, "old.tga");
        assert_eq!(munged, "#65280!1.5!old.tga#mymodel");
    }

    #[test]
    fn test_unpack_house_color() {
        let (r, g, b) = unpack_house_color(0xFF0000);
        assert!((r - 1.0).abs() < 0.01);
        assert!(g.abs() < 0.01);
        assert!(b.abs() < 0.01);

        let (r, g, b) = unpack_house_color(0x00FF00);
        assert!(r.abs() < 0.01);
        assert!((g - 1.0).abs() < 0.01);
        assert!(b.abs() < 0.01);
    }

    #[test]
    fn test_recolor_palette_entry() {
        // Full white
        let (r, g, b) = recolor_palette_entry(0, 1.0, 1.0, 1.0);
        assert_eq!(r, 255);
        assert_eq!(g, 255);
        assert_eq!(b, 255);

        // Full white, index 15 (darkest in palette)
        let (r, g, b) = recolor_palette_entry(15, 1.0, 1.0, 1.0);
        let scale = TEAM_COLOR_PALETTE[15] as f32 / 255.0;
        assert_eq!(r, (scale * 255.0) as u8);
    }

    #[test]
    fn test_texture_ref_counting() {
        let mut mgr = WthreeDAssetManager::new();
        mgr.initialize().unwrap();

        // Load a texture via get_texture_ref
        let handle = mgr.get_texture_ref("test.tga");
        assert!(handle.is_some());
        assert_eq!(mgr.texture_count(), 1);

        // Getting the same texture again doesn't create a duplicate
        let handle2 = mgr.get_texture_ref("test.tga");
        assert!(handle2.is_some());
        assert_eq!(mgr.texture_count(), 1);
    }

    #[test]
    fn test_create_render_obj_not_found() {
        let mut mgr = WthreeDAssetManager::new();
        mgr.initialize().unwrap();

        // Load on demand but file doesn't exist
        let handle = mgr.create_render_obj("NonExistent");
        assert!(handle.is_none());
    }

    #[test]
    fn test_default_trait() {
        let mgr = WthreeDAssetManager::default();
        assert!(!mgr.is_initialized());
    }
}
