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
use wgpu::util::DeviceExt;
use ww3d_assets::loaders::hierarchy_loader::W3DHierarchy;
use ww3d_assets::loaders::mesh_loader::W3DMesh;
use ww3d_assets::rendering::Vertex as W3DVertex;
use ww3d_assets::{HlodPrototype, W3DLoader, W3DModel};

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
    pub source_path: Option<PathBuf>,
    pub meshes: Vec<AssetMeshPayload>,
    pub hierarchy: Option<AssetHierarchyData>,
    pub hlod: Option<AssetHlodData>,
}

#[derive(Debug, Clone)]
pub struct AssetMeshPayload {
    pub name: String,
    pub vertex_count: u32,
    pub index_count: u32,
    pub material_name: Option<String>,
    pub texture_names: Vec<String>,
    pub bounding_min: [f32; 3],
    pub bounding_max: [f32; 3],
    pub bounding_sphere_center: [f32; 3],
    pub bounding_sphere_radius: f32,
    pub vertex_buffer: Option<Arc<wgpu::Buffer>>,
    pub index_buffer: Option<Arc<wgpu::Buffer>>,
}

#[derive(Debug, Clone)]
pub struct AssetHierarchyData {
    pub name: String,
    pub pivots: Vec<AssetHierarchyPivot>,
}

#[derive(Debug, Clone)]
pub struct AssetHierarchyPivot {
    pub name: String,
    pub parent_idx: i32,
    pub translation: [f32; 3],
    pub rotation: [f32; 4],
}

#[derive(Debug, Clone)]
pub struct AssetHlodData {
    pub name: String,
    pub hierarchy_name: String,
    pub lods: Vec<AssetHlodLod>,
    pub proxies: Vec<AssetHlodProxy>,
}

#[derive(Debug, Clone)]
pub struct AssetHlodLod {
    pub max_screen_size: f32,
    pub mesh_names: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct AssetHlodProxy {
    pub name: String,
    pub bone_index: u32,
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

    fn mesh(name: String, source_path: Option<PathBuf>, mesh: AssetMeshPayload) -> Self {
        Self {
            name,
            class_id: AssetClassId::Mesh,
            ref_count: 1,
            cache_key: None,
            object_color: 0,
            source_path,
            meshes: vec![mesh],
            hierarchy: None,
            hlod: None,
        }
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
/// r, g, b are in 0.0–1.0 range (output of unpack_house_color).
/// C++: rgb.X = houseColorScale[y] * v_color.X, then Convert_Pixel to 0–255.
fn recolor_palette_entry(index: usize, r: f32, g: f32, b: f32) -> (u8, u8, u8) {
    let scale = TEAM_COLOR_PALETTE[index] as f32 / 255.0;
    let cr = (r * scale * 255.0).clamp(0.0, 255.0) as u8;
    let cg = (g * scale * 255.0).clamp(0.0, 255.0) as u8;
    let cb = (b * scale * 255.0).clamp(0.0, 255.0) as u8;
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

/// Read back pixel data from a GPU texture via a staging buffer.
/// Returns None if the texture lacks COPY_SRC usage or readback fails.
fn readback_texture_pixels(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    source: &wgpu::Texture,
    width: u32,
    height: u32,
) -> Option<Vec<u8>> {
    if !source.usage().contains(wgpu::TextureUsages::COPY_SRC) {
        return None;
    }

    let bytes_per_pixel: u32 = 4;
    let bytes_per_row = wgpu::util::align_to(width * bytes_per_pixel, wgpu::COPY_BYTES_PER_ROW_ALIGNMENT);
    let buffer_size = (bytes_per_row * height) as u64;

    let staging = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("recolor_readback_staging"),
        size: buffer_size,
        usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });

    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("recolor_readback"),
    });
    encoder.copy_texture_to_buffer(
        wgpu::ImageCopyTexture {
            texture: source,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        wgpu::ImageCopyBuffer {
            buffer: &staging,
            layout: wgpu::ImageDataLayout {
                offset: 0,
                bytes_per_row: Some(bytes_per_row),
                rows_per_image: Some(height),
            },
        },
        wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
    );
    queue.submit(Some(encoder.finish()));
    device.poll(wgpu::Maintain::Wait);

    let slice = staging.slice(..);
    let (tx, rx) = std::sync::mpsc::channel();
    slice.map_async(wgpu::MapMode::Read, move |result| {
        let _ = tx.send(result);
    });
    device.poll(wgpu::Maintain::Wait);

    if let Ok(Ok(())) = rx.recv() {
        let data = slice.get_mapped_range();
        let padded = data.to_vec();
        drop(data);
        staging.unmap();

        let unpadded_row = width * bytes_per_pixel;
        if bytes_per_row == unpadded_row {
            Some(padded)
        } else {
            let mut pixels = Vec::with_capacity((width * height * bytes_per_pixel) as usize);
            for row in 0..height {
                let src_start = (row * bytes_per_row) as usize;
                let src_end = src_start + unpadded_row as usize;
                if src_end <= padded.len() {
                    pixels.extend_from_slice(&padded[src_start..src_end]);
                }
            }
            Some(pixels)
        }
    } else {
        None
    }
}

/// C++ remapPalette32Bit: remap the first 16 pixels (palette row) to the house color gradient.
fn remap_palette_only(pixels: &mut [u8], _width: u32, r: f32, g: f32, b: f32) {
    for i in 0..TEAM_COLOR_PALETTE_SIZE {
        let (cr, cg, cb) = recolor_palette_entry(i, r, g, b);
        let offset = i * 4;
        if offset + 3 < pixels.len() {
            pixels[offset] = cr;
            pixels[offset + 1] = cg;
            pixels[offset + 2] = cb;
            pixels[offset + 3] = 255;
        }
    }
}

/// C++ remapAlphaTexture32Bit: blend house color into every pixel using inverted alpha channel.
/// alpha=0 → full house color, alpha=255 → no house color (inverted).
fn remap_alpha_texture(pixels: &mut [u8], width: u32, height: u32, r: f32, g: f32, b: f32) {
    let r255 = r * 255.0;
    let g255 = g * 255.0;
    let b255 = b * 255.0;
    for y in 0..height {
        for x in 0..width {
            let offset = ((y * width + x) as usize) * 4;
            if offset + 3 >= pixels.len() {
                break;
            }
            let pixel_alpha = 255 - pixels[offset + 3];
            if pixel_alpha != 0 {
                let fa = pixel_alpha as f32 / 255.0;
                let fi = 1.0 - fa;
                pixels[offset] = (fa * r255 + fi * pixels[offset] as f32).clamp(0.0, 255.0) as u8;
                pixels[offset + 1] = (fa * g255 + fi * pixels[offset + 1] as f32).clamp(0.0, 255.0) as u8;
                pixels[offset + 2] = (fa * b255 + fi * pixels[offset + 2] as f32).clamp(0.0, 255.0) as u8;
            }
            pixels[offset + 3] = 255;
        }
    }
}

/// Dispatch to the correct remap mode based on texture name convention.
/// C++ Recolor_Texture_One_Time checks name[3] for 'D' or 'A'.
fn remap_texture_pixels(
    pixels: &mut [u8],
    width: u32,
    height: u32,
    source_name: &str,
    r: f32,
    g: f32,
    b: f32,
) {
    let ch = source_name.chars().nth(3);
    match ch {
        Some('D' | 'd') => remap_palette_only(pixels, width, r, g, b),
        Some('A' | 'a') => remap_alpha_texture(pixels, width, height, r, g, b),
        _ => {}
    }
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

    /// Texture search paths for resolving texture filenames.
    /// C++ uses WW3D texture manager search paths (Art/Textures, Data/Art/Textures, etc.)
    texture_search_paths: Vec<PathBuf>,
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
            texture_search_paths: default_texture_search_paths(),
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

    /// Set texture search paths for file resolution.
    pub fn set_texture_search_paths(&mut self, paths: Vec<PathBuf>) {
        self.texture_search_paths = paths;
    }

    /// Add a texture search path.
    pub fn add_texture_search_path(&mut self, path: PathBuf) {
        if !self.texture_search_paths.contains(&path) {
            self.texture_search_paths.push(path);
        }
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

    pub fn find_prototype_by_handle(&self, handle_id: u64) -> Option<&AssetPrototype> {
        self.prototypes
            .values()
            .find(|prototype| hash_name(&prototype.name) == handle_id)
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
        let filename = _filename;
        let base_name = _base_name;
        let path = Path::new(filename);
        if !path.exists() {
            return false;
        }

        let model = match W3DLoader::load(path) {
            Ok(model) => model,
            Err(err) => {
                log::warn!("Failed to parse W3D file {}: {}", filename, err);
                return false;
            }
        };

        let source_path = Some(path.to_path_buf());
        let hierarchies = model
            .hierarchies
            .iter()
            .map(|hierarchy| {
                (
                    hierarchy.header.name.to_ascii_lowercase(),
                    Self::convert_hierarchy(hierarchy),
                )
            })
            .collect::<HashMap<_, _>>();

        let mesh_payloads = model
            .meshes
            .iter()
            .filter_map(|mesh| match self.build_mesh_payload(mesh) {
                Some(payload) => Some((mesh.header.mesh_name.to_ascii_lowercase(), payload)),
                None => {
                    log::warn!(
                        "Skipping empty mesh '{}' from {}",
                        mesh.header.mesh_name,
                        filename
                    );
                    None
                }
            })
            .collect::<HashMap<_, _>>();

        if mesh_payloads.is_empty() && model.hlods.is_empty() {
            return false;
        }

        for mesh in &model.meshes {
            let mesh_name = mesh.header.mesh_name.trim_end_matches('\0');
            if mesh_name.is_empty() {
                continue;
            }

            if let Some(payload) = mesh_payloads.get(&mesh_name.to_ascii_lowercase()) {
                let mut proto = AssetPrototype::mesh(
                    mesh_name.to_string(),
                    source_path.clone(),
                    payload.clone(),
                );
                proto.hierarchy = Self::find_mesh_hierarchy(mesh, &model, &hierarchies);
                self.add_prototype(proto);
            }
        }

        for hlod in &model.hlods {
            let proto = self.build_hlod_prototype(hlod, &mesh_payloads, &hierarchies, source_path.clone());
            self.add_prototype(proto);
        }

        if !self.prototypes.contains_key(base_name) {
            if let Some(hlod) = model
                .hlods
                .iter()
                .find(|hlod| hlod.name.eq_ignore_ascii_case(base_name))
                .or_else(|| model.hlods.first())
            {
                let alias = self.build_hlod_prototype(hlod, &mesh_payloads, &hierarchies, source_path.clone());
                self.prototypes.insert(base_name.to_string(), AssetPrototype { name: base_name.to_string(), ..alias });
            } else if let Some(mesh) = model
                .meshes
                .iter()
                .find(|mesh| mesh.header.mesh_name.eq_ignore_ascii_case(base_name))
                .or_else(|| model.meshes.first())
            {
                if let Some(payload) = mesh_payloads.get(&mesh.header.mesh_name.to_ascii_lowercase()) {
                    let mut proto = AssetPrototype::mesh(
                        base_name.to_string(),
                        source_path.clone(),
                        payload.clone(),
                    );
                    proto.hierarchy = Self::find_mesh_hierarchy(mesh, &model, &hierarchies);
                    self.prototypes.insert(base_name.to_string(), proto);
                }
            }
        }

        self.prototypes.contains_key(base_name)
    }

    fn build_mesh_payload(&self, mesh: &W3DMesh) -> Option<AssetMeshPayload> {
        if mesh.vertices.is_empty() || mesh.triangles.is_empty() {
            return None;
        }

        let vertices = mesh
            .vertices
            .iter()
            .enumerate()
            .map(|(index, position)| {
                let normal = mesh.normals.get(index).copied().unwrap_or_default();
                let uv = mesh.tex_coords.get(index).copied().unwrap_or_default();

                W3DVertex {
                    position: *position,
                    normal,
                    uv,
                    color: glam::Vec4::ONE,
                }
            })
            .collect::<Vec<_>>();

        let indices = mesh
            .triangles
            .iter()
            .flat_map(|triangle| triangle.iter().copied())
            .collect::<Vec<_>>();

        let (vertex_buffer, index_buffer) = if let Some(device) = &self.device {
            let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some(&format!("W3D Mesh Vertex Buffer: {}", mesh.header.mesh_name)),
                contents: bytemuck::cast_slice(&vertices),
                usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            });
            let index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some(&format!("W3D Mesh Index Buffer: {}", mesh.header.mesh_name)),
                contents: bytemuck::cast_slice(&indices),
                usage: wgpu::BufferUsages::INDEX | wgpu::BufferUsages::COPY_DST,
            });
            (Some(Arc::new(vertex_buffer)), Some(Arc::new(index_buffer)))
        } else {
            (None, None)
        };

        Some(AssetMeshPayload {
            name: mesh.header.mesh_name.clone(),
            vertex_count: vertices.len() as u32,
            index_count: indices.len() as u32,
            material_name: mesh
                .textures
                .first()
                .map(|texture| texture.name.clone())
                .filter(|name| !name.is_empty()),
            texture_names: mesh.textures.iter().map(|texture| texture.name.clone()).collect(),
            bounding_min: mesh.header.min.to_array(),
            bounding_max: mesh.header.max.to_array(),
            bounding_sphere_center: mesh.header.sph_center.to_array(),
            bounding_sphere_radius: mesh.header.sph_radius,
            vertex_buffer,
            index_buffer,
        })
    }

    fn convert_hierarchy(hierarchy: &W3DHierarchy) -> AssetHierarchyData {
        AssetHierarchyData {
            name: hierarchy.header.name.clone(),
            pivots: hierarchy
                .pivots
                .iter()
                .map(|pivot| AssetHierarchyPivot {
                    name: pivot.name.clone(),
                    parent_idx: pivot.parent_idx,
                    translation: pivot.translation.to_array(),
                    rotation: [pivot.rotation.x, pivot.rotation.y, pivot.rotation.z, pivot.rotation.w],
                })
                .collect(),
        }
    }

    fn find_mesh_hierarchy(
        mesh: &W3DMesh,
        model: &W3DModel,
        hierarchies: &HashMap<String, AssetHierarchyData>,
    ) -> Option<AssetHierarchyData> {
        let container_name = mesh.header.container_name.trim_end_matches('\0');
        if !container_name.is_empty() {
            if let Some(hierarchy) = hierarchies.get(&container_name.to_ascii_lowercase()) {
                return Some(hierarchy.clone());
            }
        }

        model
            .hierarchies
            .first()
            .and_then(|hierarchy| hierarchies.get(&hierarchy.header.name.to_ascii_lowercase()))
            .cloned()
    }

    fn build_hlod_prototype(
        &self,
        hlod: &HlodPrototype,
        mesh_payloads: &HashMap<String, AssetMeshPayload>,
        hierarchies: &HashMap<String, AssetHierarchyData>,
        source_path: Option<PathBuf>,
    ) -> AssetPrototype {
        let mesh_names = hlod
            .lods
            .iter()
            .flat_map(|lod| lod.models.iter().map(|model| model.name.to_ascii_lowercase()))
            .collect::<Vec<_>>();

        let meshes = mesh_names
            .iter()
            .filter_map(|mesh_name| mesh_payloads.get(mesh_name).cloned())
            .collect::<Vec<_>>();

        let hierarchy = hierarchies
            .get(&hlod.hierarchy_name.to_ascii_lowercase())
            .cloned();

        AssetPrototype {
            name: hlod.name.clone(),
            class_id: AssetClassId::Hlod,
            ref_count: 1,
            cache_key: None,
            object_color: 0,
            source_path,
            meshes,
            hierarchy,
            hlod: Some(AssetHlodData {
                name: hlod.name.clone(),
                hierarchy_name: hlod.hierarchy_name.clone(),
                lods: hlod
                    .lods
                    .iter()
                    .map(|lod| AssetHlodLod {
                        max_screen_size: lod.max_screen_size,
                        mesh_names: lod.models.iter().map(|model| model.name.clone()).collect(),
                    })
                    .collect(),
                proxies: hlod
                    .proxy_entries
                    .iter()
                    .map(|proxy| AssetHlodProxy {
                        name: proxy.name.clone(),
                        bone_index: proxy.bone_index,
                    })
                    .collect(),
            }),
        }
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
        // C++ parity: TextureClass::Init from file, uploads to GPU.
        // Step 1: resolve the texture file on disk
        let resolved = match self.resolve_texture_file(name) {
            Some(p) => p,
            None => {
                log::debug!("Texture '{}' not found on disk, creating placeholder", name);
                return self.create_placeholder_texture(name);
            }
        };

        // Step 2: read file bytes
        let bytes = match std::fs::read(&resolved) {
            Ok(b) => b,
            Err(e) => {
                log::warn!("Failed to read texture '{}': {}", resolved.display(), e);
                return self.create_placeholder_texture(name);
            }
        };

        // Step 3: decode based on format
        let lower_name = name.to_ascii_lowercase();
        if lower_name.ends_with(".dds") {
            self.load_dds_texture(name, &bytes)
        } else {
            // TGA, PNG, BMP, JPEG — use the `image` crate
            self.load_image_texture(name, &bytes)
        }
    }

    /// Create a 1x1 magenta placeholder texture (fallback for missing textures).
    fn create_placeholder_texture(&mut self, name: &str) -> bool {
        let (wgpu_texture, wgpu_view, width, height) = if let (Some(device), Some(queue)) =
            (&self.device, &self.queue)
        {
            let texture = device.create_texture(&wgpu::TextureDescriptor {
                label: Some(&format!("placeholder_{}", name)),
                size: wgpu::Extent3d { width: 1, height: 1, depth_or_array_layers: 1 },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Rgba8UnormSrgb,
                usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
                view_formats: &[],
            });
            let pixel: [u8; 4] = [255, 0, 255, 255]; // magenta
            queue.write_texture(
                wgpu::ImageCopyTexture {
                    texture: &texture,
                    mip_level: 0,
                    origin: wgpu::Origin3d::ZERO,
                    aspect: wgpu::TextureAspect::All,
                },
                &pixel,
                wgpu::ImageDataLayout {
                    offset: 0,
                    bytes_per_row: Some(4),
                    rows_per_image: Some(1),
                },
                wgpu::Extent3d { width: 1, height: 1, depth_or_array_layers: 1 },
            );
            let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
            (Some(Arc::new(texture)), Some(Arc::new(view)), 1, 1)
        } else {
            (None, None, 0, 0)
        };

        let tex = AssetTexture {
            name: name.to_string(),
            wgpu_texture,
            wgpu_view,
            ref_count: 1,
            width,
            height,
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
        };
        self.textures.insert(name.to_string(), tex);
        true
    }

    /// Load a standard image (TGA/PNG/BMP/JPEG) using the `image` crate and upload to GPU.
    fn load_image_texture(&mut self, name: &str, bytes: &[u8]) -> bool {
        let img = match image::load_from_memory(bytes) {
            Ok(i) => i,
            Err(e) => {
                log::warn!("Failed to decode texture '{}': {}", name, e);
                return self.create_placeholder_texture(name);
            }
        };

        let (width, height) = img.dimensions();
        let rgba = img.to_rgba8();
        let mip_level_count = calculate_mip_levels(width, height);

        let (wgpu_texture, wgpu_view) = if let (Some(device), Some(queue)) =
            (&self.device, &self.queue)
        {
            let texture = device.create_texture(&wgpu::TextureDescriptor {
                label: Some(&format!("texture_{}", name)),
                size: wgpu::Extent3d { width, height, depth_or_array_layers: 1 },
                mip_level_count,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Rgba8UnormSrgb,
                usage: wgpu::TextureUsages::TEXTURE_BINDING
                    | wgpu::TextureUsages::COPY_DST
                    | wgpu::TextureUsages::COPY_SRC
                    | wgpu::TextureUsages::RENDER_ATTACHMENT,
                view_formats: &[],
            });

            // Upload mip level 0 (full resolution)
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
                wgpu::Extent3d { width, height, depth_or_array_layers: 1 },
            );

            // Generate and upload remaining mip levels
            upload_mip_levels(&texture, &rgba, width, height, device, queue);

            let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
            (Some(Arc::new(texture)), Some(Arc::new(view)))
        } else {
            (None, None)
        };

        let tex = AssetTexture {
            name: name.to_string(),
            wgpu_texture,
            wgpu_view,
            ref_count: 1,
            width,
            height,
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
        };
        self.textures.insert(name.to_string(), tex);
        true
    }

    /// Load a DDS texture with DXT/BCn compressed data and upload to GPU.
    fn load_dds_texture(&mut self, name: &str, bytes: &[u8]) -> bool {
        let dds = match DdsTexture::parse(bytes) {
            Ok(d) => d,
            Err(e) => {
                log::warn!("Failed to parse DDS texture '{}': {}", name, e);
                return self.create_placeholder_texture(name);
            }
        };

        let (wgpu_texture, wgpu_view) = if let (Some(device), Some(queue)) =
            (&self.device, &self.queue)
        {
            let mip_level_count = calculate_mip_levels(dds.width, dds.height)
                .min(dds.mip_level_count.max(1));

            let texture = device.create_texture(&wgpu::TextureDescriptor {
                label: Some(&format!("dds_texture_{}", name)),
                size: wgpu::Extent3d { width: dds.width, height: dds.height, depth_or_array_layers: 1 },
                mip_level_count,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: dds.wgpu_format,
                usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
                view_formats: &[],
            });

            // Upload each available mip level from the DDS data
            let mut offset = dds.data_offset;
            let mut mip_w = dds.width;
            let mut mip_h = dds.height;

            for mip in 0..mip_level_count {
                let mip_size = dds.mip_data_size(mip_w, mip_h);
                if offset + mip_size > bytes.len() {
                    break;
                }
                let mip_data = &bytes[offset..offset + mip_size];
                let (bytes_per_row, rows_per_image) = dds.mip_layout(mip_w, mip_h);

                queue.write_texture(
                    wgpu::ImageCopyTexture {
                        texture: &texture,
                        mip_level: mip,
                        origin: wgpu::Origin3d::ZERO,
                        aspect: wgpu::TextureAspect::All,
                    },
                    mip_data,
                    wgpu::ImageDataLayout {
                        offset: 0,
                        bytes_per_row: Some(bytes_per_row),
                        rows_per_image: Some(rows_per_image),
                    },
                    wgpu::Extent3d { width: mip_w, height: mip_h, depth_or_array_layers: 1 },
                );

                offset += mip_size;
                mip_w = (mip_w + 1) / 2;
                mip_h = (mip_h + 1) / 2;
            }

            let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
            (Some(Arc::new(texture)), Some(Arc::new(view)))
        } else {
            (None, None)
        };

        let tex = AssetTexture {
            name: name.to_string(),
            wgpu_texture,
            wgpu_view,
            ref_count: 1,
            width: dds.width,
            height: dds.height,
            format: dds.wgpu_format,
        };
        self.textures.insert(name.to_string(), tex);
        true
    }

    /// Resolve a texture name to a file path using search paths.
    fn resolve_texture_file(&self, name: &str) -> Option<PathBuf> {
        // Try the name as-is first
        let p = Path::new(name);
        if p.exists() {
            return Some(p.to_path_buf());
        }

        // Try each search path with the name as-is
        for base in &self.texture_search_paths {
            let candidate = base.join(name);
            if candidate.exists() {
                return Some(candidate);
            }
        }

        // Try with common texture extensions if the name has no extension
        let stem = Path::new(name)
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or(name);

        let extensions = ["tga", "dds", "png", "bmp", "jpg"];
        for base in &self.texture_search_paths {
            for ext in &extensions {
                let candidate = base.join(format!("{}.{}", stem, ext));
                if candidate.exists() {
                    return Some(candidate);
                }
            }
        }

        None
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
        if !self.textures.contains_key(name) {
            return None;
        }

        // C++: procedural textures (name starts with '!') return NULL
        if name.starts_with('!') {
            return None;
        }

        let (r, g, b) = unpack_house_color(color);

        // Clone source data to release the borrow before insertion
        let source = self.textures.get(name)?;
        let width = source.width;
        let height = source.height;
        let format = source.format;
        let source_gpu = source.wgpu_texture.clone();
        drop(source);

        let munged = munge_texture_name(name, color);
        let mut new_tex = AssetTexture {
            name: munged.clone(),
            wgpu_texture: None,
            wgpu_view: None,
            ref_count: 1,
            width,
            height,
            format,
        };

        if let (Some(device), Some(queue), Some(src_tex)) =
            (&self.device, &self.queue, &source_gpu)
        {
            // Only read back uncompressed RGBA8 textures
            if format == wgpu::TextureFormat::Rgba8UnormSrgb {
                if let Some(mut pixels) =
                    readback_texture_pixels(device, queue, src_tex, width, height)
                {
                    remap_texture_pixels(&mut pixels, width, height, name, r, g, b);

                    let new_texture = device.create_texture(&wgpu::TextureDescriptor {
                        label: Some(&format!("recolor_{}", munged)),
                        size: wgpu::Extent3d {
                            width,
                            height,
                            depth_or_array_layers: 1,
                        },
                        mip_level_count: 1,
                        sample_count: 1,
                        dimension: wgpu::TextureDimension::D2,
                        format: wgpu::TextureFormat::Rgba8UnormSrgb,
                        usage: wgpu::TextureUsages::TEXTURE_BINDING
                            | wgpu::TextureUsages::COPY_DST
                            | wgpu::TextureUsages::COPY_SRC,
                        view_formats: &[],
                    });

                    let bytes_per_row = width * 4;
                    queue.write_texture(
                        wgpu::ImageCopyTexture {
                            texture: &new_texture,
                            mip_level: 0,
                            origin: wgpu::Origin3d::ZERO,
                            aspect: wgpu::TextureAspect::All,
                        },
                        &pixels,
                        wgpu::ImageDataLayout {
                            offset: 0,
                            bytes_per_row: Some(bytes_per_row),
                            rows_per_image: Some(height),
                        },
                        wgpu::Extent3d {
                            width,
                            height,
                            depth_or_array_layers: 1,
                        },
                    );

                    let view = new_texture.create_view(&wgpu::TextureViewDescriptor::default());
                    new_tex.wgpu_texture = Some(Arc::new(new_texture));
                    new_tex.wgpu_view = Some(Arc::new(view));
                }
            }
        }

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

/// Calculate number of mip levels for a given texture size.
fn calculate_mip_levels(width: u32, height: u32) -> u32 {
    let max_dim = width.max(height);
    if max_dim == 0 { 1 } else { 32 - max_dim.leading_zeros() }
}

/// Generate and upload mip levels 1..N from the base RGBA image data.
fn upload_mip_levels(
    texture: &wgpu::Texture,
    base_rgba: &image::RgbaImage,
    base_width: u32,
    base_height: u32,
    device: &wgpu::Device,
    queue: &wgpu::Queue,
) {
    let total_mips = calculate_mip_levels(base_width, base_height);
    if total_mips <= 1 {
        return;
    }

    let encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("mip_upload_encoder"),
    });

    // Generate each mip level by downscaling with the `image` crate
    let mut prev_img = base_rgba.clone();
    let mut mip_w = base_width;
    let mut mip_h = base_height;

    for mip in 1..total_mips {
        mip_w = (mip_w + 1) / 2;
        mip_h = (mip_h + 1) / 2;
        if mip_w == 0 || mip_h == 0 {
            break;
        }

        let mip_img = image::imageops::resize(
            &prev_img,
            mip_w,
            mip_h,
            image::imageops::FilterType::Triangle,
        );

        queue.write_texture(
            wgpu::ImageCopyTexture {
                texture,
                mip_level: mip,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &mip_img,
            wgpu::ImageDataLayout {
                offset: 0,
                bytes_per_row: Some(4 * mip_w),
                rows_per_image: Some(mip_h),
            },
            wgpu::Extent3d { width: mip_w, height: mip_h, depth_or_array_layers: 1 },
        );

        prev_img = mip_img;
    }

    drop(encoder);
}

/// Default texture search paths matching C++ WW3D texture manager paths.
fn default_texture_search_paths() -> Vec<PathBuf> {
    let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    vec![
        cwd.join("Art/Textures"),
        cwd.join("Data/Art/Textures"),
        cwd.join("Textures"),
        cwd.join("Data/Textures"),
        cwd.join("assets/Textures"),
        cwd.join("assets/Art/Textures"),
        cwd.clone(),
    ]
}

// ---------------------------------------------------------------------------
// DDS texture parsing
// ---------------------------------------------------------------------------

/// DDS pixel format flags
const DDPF_FOURCC: u32 = 0x4;
const DDPF_RGB: u32 = 0x40;

/// Minimal parsed DDS texture header + metadata.
struct DdsTexture {
    width: u32,
    height: u32,
    mip_level_count: u32,
    wgpu_format: wgpu::TextureFormat,
    block_size: u32,
    data_offset: usize,
}

impl DdsTexture {
    /// Parse a DDS header from raw bytes. Supports DXT1/BC1, DXT3/BC2, DXT5/BC3, and uncompressed RGBA.
    fn parse(bytes: &[u8]) -> Result<Self, String> {
        if bytes.len() < 128 {
            return Err("DDS file too small".to_string());
        }

        // Validate magic number "DDS "
        if &bytes[0..4] != b"DDS " {
            return Err("Invalid DDS magic number".to_string());
        }

        // DDS_HEADER fields (offset 4)
        let height = u32_from_le(&bytes[12..16]);
        let width = u32_from_le(&bytes[16..20]);
        let mip_level_count = u32_from_le(&bytes[28..32]).max(1);

        // DDS_PIXELFORMAT (offset 76 within header, offset 80 from file start)
        let pf_size = u32_from_le(&bytes[76..80]);
        if pf_size != 32 {
            return Err(format!("Unexpected DDS pixel format size: {}", pf_size));
        }
        let pf_flags = u32_from_le(&bytes[80..84]);
        let fourcc = &bytes[84..88];

        let (wgpu_format, block_size) = if pf_flags & DDPF_FOURCC != 0 {
            match fourcc {
                b"DXT1" => (wgpu::TextureFormat::Bc1RgbaUnormSrgb, 8),
                b"DXT3" => (wgpu::TextureFormat::Bc2RgbaUnormSrgb, 16),
                b"DXT5" => (wgpu::TextureFormat::Bc3RgbaUnormSrgb, 16),
                b"BC4U" | b"ATI1" => (wgpu::TextureFormat::Bc4RUnorm, 8),
                b"BC5U" | b"ATI2" => (wgpu::TextureFormat::Bc5RgUnorm, 16),
                _ => return Err(format!("Unsupported DDS FourCC: {:?}", std::str::from_utf8(fourcc).unwrap_or("????"))),
            }
        } else if pf_flags & DDPF_RGB != 0 {
            // Uncompressed RGBA
            (wgpu::TextureFormat::Rgba8UnormSrgb, 0)
        } else {
            return Err("Unsupported DDS pixel format".to_string());
        };

        let data_offset = 128; // magic(4) + header(124)

        Ok(Self {
            width,
            height,
            mip_level_count,
            wgpu_format,
            block_size,
            data_offset,
        })
    }

    /// Calculate the byte size of compressed mip data for given dimensions.
    fn mip_data_size(&self, width: u32, height: u32) -> usize {
        if self.block_size == 0 {
            // Uncompressed RGBA
            return (width * height * 4) as usize;
        }
        let blocks_x = (width + 3) / 4;
        let blocks_y = (height + 3) / 4;
        (blocks_x * blocks_y * self.block_size) as usize
    }

    /// Calculate bytes_per_row and rows_per_image for a mip level.
    fn mip_layout(&self, width: u32, height: u32) -> (u32, u32) {
        if self.block_size == 0 {
            return (width * 4, height);
        }
        let blocks_x = (width + 3) / 4;
        let blocks_y = (height + 3) / 4;
        (blocks_x * self.block_size, blocks_y)
    }
}

/// Read a little-endian u32 from a byte slice.
fn u32_from_le(bytes: &[u8]) -> u32 {
    u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]])
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
            source_path: None,
            meshes: Vec::new(),
            hierarchy: None,
            hlod: None,
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
            source_path: None,
            meshes: Vec::new(),
            hierarchy: None,
            hlod: None,
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
            source_path: None,
            meshes: Vec::new(),
            hierarchy: None,
            hlod: None,
        });
        mgr.add_prototype(AssetPrototype {
            name: "B".to_string(),
            class_id: AssetClassId::Hlod,
            ref_count: 3,
            cache_key: None,
            object_color: 0,
            source_path: None,
            meshes: Vec::new(),
            hierarchy: None,
            hlod: None,
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
