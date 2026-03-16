//! Texture file cache – faithful WGPU-era implementation of the original DX8 cache.
//!
//! The C++ renderer persisted preprocessed textures inside a tag-block based cache file so
//! subsequent runs could bypass expensive decoding steps. Each entry stored a block header,
//! mip-map offset table, and raw (optionally compressed) texel data.  This module mirrors that
//! behaviour closely while integrating with the Rust renderer infrastructure.

use crate::core::error::{Error, RendererResult};
use crate::core::ww3dformat::WW3DFormat;
use crate::rendering::texture_decode::{
    decode_texture_file, TextureData, TextureDataKind, TextureMipLevel,
};
use crate::rendering::texture_system::texture_base::{PoolType, TexAssetType, TextureBaseClass};
use std::collections::HashMap;
use std::fs::{File, OpenOptions};
use std::io::{ErrorKind, Read, Seek, SeekFrom, Write};
use std::ops::{Deref, DerefMut};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, OnceLock};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use ww3d_core::ww3d::WW3D;

const CACHE_MAGIC: [u8; 4] = *b"WWTC";
const CACHE_VERSION: u32 = 20000814; // Matches FileHeader::TCF_VERSION in the legacy renderer.
const MAX_CACHED_SURFACES: usize = 16;

/// Minimal stand-in for the legacy `srColorSurface` interface.
#[derive(Debug, Clone)]
pub struct SrColorSurfaceIFace {
    pub width: u32,
    pub height: u32,
    pub format: WW3DFormat,
    pub bytes: Vec<u8>,
}

impl SrColorSurfaceIFace {
    pub fn new(width: u32, height: u32, format: WW3DFormat, bytes: Vec<u8>) -> Self {
        Self {
            width,
            height,
            format,
            bytes,
        }
    }

    pub fn data_size(&self) -> usize {
        self.bytes.len()
    }

    pub fn copy_from(&mut self, source: &SrColorSurfaceIFace) {
        if self.width == source.width && self.height == source.height {
            let copy_len = self.bytes.len().min(source.bytes.len());
            self.bytes[..copy_len].copy_from_slice(&source.bytes[..copy_len]);
            return;
        }

        // Faithful intent: legacy surface copy scales source mip data into destination.
        // We use deterministic nearest-neighbour scaling for parity-safe behaviour.
        let src_bpp = if source.width == 0 || source.height == 0 {
            0
        } else {
            source.bytes.len() / (source.width as usize * source.height as usize)
        };
        let dst_bpp = if self.width == 0 || self.height == 0 {
            0
        } else {
            self.bytes.len() / (self.width as usize * self.height as usize)
        };
        if src_bpp == 0 || src_bpp != dst_bpp {
            return;
        }

        let src_w = source.width.max(1) as usize;
        let src_h = source.height.max(1) as usize;
        let dst_w = self.width.max(1) as usize;
        let dst_h = self.height.max(1) as usize;

        for y in 0..dst_h {
            let sy = (y * src_h) / dst_h;
            for x in 0..dst_w {
                let sx = (x * src_w) / dst_w;
                let src_offset = (sy * src_w + sx) * src_bpp;
                let dst_offset = (y * dst_w + x) * dst_bpp;
                self.bytes[dst_offset..dst_offset + dst_bpp]
                    .copy_from_slice(&source.bytes[src_offset..src_offset + src_bpp]);
            }
        }
    }
}

/// Rust equivalent of `srTextureIFace::MultiRequest`.
#[derive(Debug, Clone)]
pub struct TextureMultiRequest {
    pub levels: Vec<Option<SrColorSurfaceIFace>>,
    pub large_lod: u32,
    pub small_lod: u32,
}

impl TextureMultiRequest {
    pub fn new(max_lod: usize) -> Self {
        Self {
            levels: vec![None; max_lod.max(1)],
            large_lod: 0,
            small_lod: 0,
        }
    }
}

impl Default for TextureMultiRequest {
    fn default() -> Self {
        Self::new(MAX_CACHED_SURFACES)
    }
}

/// Runtime configuration for the cache.
#[derive(Debug, Clone)]
pub struct TextureCacheConfig {
    pub max_memory_mb: u64,
    pub max_entries: usize,
    pub unused_timeout_seconds: f64,
    pub enable_hot_reload: bool,
}

impl Default for TextureCacheConfig {
    fn default() -> Self {
        Self {
            max_memory_mb: 256,
            max_entries: 1000,
            unused_timeout_seconds: 30.0,
            enable_hot_reload: false,
        }
    }
}

/// Header information mirrored from the DX8 cache.
#[derive(Debug, Clone)]
struct TextureBlockHeader {
    file_time: u64,
    num_mipmaps: u32,
    largest_width: u32,
    largest_height: u32,
    source_width: u32,
    source_height: u32,
    source_format: WW3DFormat,
    stored_format: WW3DFormat,
    asset_type: TexAssetType,
}

#[derive(Debug, Clone)]
struct MipOffset {
    relative_offset: u64,
    size: u64,
}

#[derive(Debug)]
struct TextureCacheEntry {
    header: TextureBlockHeader,
    offsets: Vec<MipOffset>,
    data_offset: u64,
    data_size: u64,
    cached_surfaces: Vec<Option<SrColorSurfaceIFace>>,
    texture: Option<Arc<TextureBaseClass>>,
    ref_count: AtomicU32,
    last_access_time: SystemTime,
    file_time: u64,
}

impl TextureCacheEntry {
    fn new(
        header: TextureBlockHeader,
        offsets: Vec<MipOffset>,
        data_offset: u64,
        data_size: u64,
    ) -> Self {
        let mip_slots = header.num_mipmaps.max(1) as usize;
        Self {
            file_time: header.file_time,
            header,
            offsets,
            data_offset,
            data_size,
            cached_surfaces: vec![None; mip_slots],
            texture: None,
            ref_count: AtomicU32::new(0),
            last_access_time: {
                let sync = WW3D::sync_time() as u64;
                if sync > 0 {
                    UNIX_EPOCH + Duration::from_millis(sync)
                } else {
                    SystemTime::now()
                }
            },
        }
    }

    fn add_ref(&self) {
        self.ref_count.fetch_add(1, Ordering::SeqCst);
    }

    fn release_ref(&self) -> u32 {
        let previous = self.ref_count.fetch_sub(1, Ordering::SeqCst);
        previous.saturating_sub(1)
    }

    fn is_unused(&self) -> bool {
        self.ref_count.load(Ordering::SeqCst) == 0
    }

    fn touch(&mut self) {
        let sync = WW3D::sync_time() as u64;
        if sync > 0 {
            self.last_access_time = UNIX_EPOCH + Duration::from_millis(sync);
        } else {
            self.last_access_time = SystemTime::now();
        }
    }

    fn drop_texture(&mut self) -> Option<Arc<TextureBaseClass>> {
        self.texture.take()
    }
}

/// File-backed texture cache closely mirroring the DX8 behaviour.
pub struct TextureFileCache {
    file_prefix: String,
    file: Option<File>,
    entries: HashMap<String, TextureCacheEntry>,
    config: TextureCacheConfig,
    total_memory_usage: u64,
    current_texture: Option<String>,
}

impl TextureFileCache {
    /// Create a cache with default configuration.
    pub fn new(file_prefix: &str) -> Self {
        Self::new_with_config(file_prefix, TextureCacheConfig::default())
    }

    /// Create a cache with explicit configuration.
    pub fn new_with_config(file_prefix: &str, config: TextureCacheConfig) -> Self {
        Self {
            file_prefix: file_prefix.to_string(),
            file: None,
            entries: HashMap::new(),
            config,
            total_memory_usage: 0,
            current_texture: None,
        }
    }

    fn key_for(name: &str) -> String {
        name.to_ascii_lowercase()
    }

    fn cache_path(&self) -> PathBuf {
        let mut path = PathBuf::from(&self.file_prefix);
        if path.extension().is_none() {
            path.set_extension("tfc");
        }
        path
    }

    fn ensure_file_open(&mut self) -> RendererResult<()> {
        if self.file.is_some() {
            return Ok(());
        }

        let cache_path = self.cache_path();
        if let Some(parent) = cache_path.parent() {
            if !parent.as_os_str().is_empty() {
                std::fs::create_dir_all(parent).map_err(Error::from)?;
            }
        }

        let mut file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .open(&cache_path)?;

        if file.metadata()?.len() == 0 {
            file.write_all(&CACHE_MAGIC)?;
            write_u32(&mut file, CACHE_VERSION)?;
            file.flush()?;
        }

        self.entries.clear();
        self.total_memory_usage = 0;
        if self.scan_index(&mut file).is_err() {
            Self::reset_file_internal(&mut file)?;
            self.entries.clear();
            self.total_memory_usage = 0;
            self.scan_index(&mut file)?;
        }
        file.seek(SeekFrom::End(0))?;

        self.file = Some(file);
        Ok(())
    }

    fn reset_file_internal(file: &mut File) -> RendererResult<()> {
        file.set_len(0)?;
        file.seek(SeekFrom::Start(0))?;
        file.write_all(&CACHE_MAGIC)?;
        write_u32(file, CACHE_VERSION)?;
        file.flush()?;
        Ok(())
    }

    fn scan_index(&mut self, file: &mut File) -> RendererResult<()> {
        file.seek(SeekFrom::Start(0))?;

        let mut magic = [0u8; 4];
        file.read_exact(&mut magic)?;
        if magic != CACHE_MAGIC {
            return Err(Error::InvalidData(
                "texture cache header magic mismatch".into(),
            ));
        }

        let version = read_u32(file)?;
        if version != CACHE_VERSION {
            return Err(Error::InvalidData(format!(
                "unsupported texture cache version {version} (expected {CACHE_VERSION})"
            )));
        }

        loop {
            let name_len = match read_u32_optional(file)? {
                Some(v) => v as usize,
                None => break,
            };
            if name_len == 0 {
                return Err(Error::InvalidData(
                    "texture cache entry has empty name".into(),
                ));
            }

            let mut name_bytes = vec![0u8; name_len];
            file.read_exact(&mut name_bytes)?;
            let name = String::from_utf8(name_bytes)
                .map_err(|_| Error::InvalidData("texture cache name is not UTF-8".into()))?;

            let header = read_block_header(file)?;
            let mip_count = header.num_mipmaps.max(1) as usize;

            let mut offsets = Vec::with_capacity(mip_count);
            for _ in 0..mip_count {
                let relative_offset = read_u64(file)?;
                let size = read_u64(file)?;
                offsets.push(MipOffset {
                    relative_offset,
                    size,
                });
            }

            let data_offset = file.stream_position()?;
            let total_size: u64 = offsets.iter().map(|o| o.size).sum();
            file.seek(SeekFrom::Current(total_size as i64))?;

            let entry = TextureCacheEntry::new(header.clone(), offsets, data_offset, total_size);
            self.entries.insert(Self::key_for(&name), entry);
        }

        Ok(())
    }

    fn reopen_file(&mut self) -> RendererResult<()> {
        self.file = None;
        self.ensure_file_open()
    }

    fn find_source_path(&self, texturename: &str) -> Option<PathBuf> {
        let requested = Path::new(texturename);
        if requested.exists() {
            return Some(requested.to_path_buf());
        }

        let prefix_path = Path::new(&self.file_prefix);
        if prefix_path.is_dir() {
            let candidate = prefix_path.join(texturename);
            if candidate.exists() {
                return Some(candidate);
            }
        }

        if let Some(parent) = prefix_path.parent() {
            let candidate = parent.join(texturename);
            if candidate.exists() {
                return Some(candidate);
            }
        }

        None
    }

    fn add_entry(
        &mut self,
        name: &str,
        texture: TextureBaseClass,
        header: TextureBlockHeader,
        offsets: Vec<MipOffset>,
        data: Vec<u8>,
    ) -> RendererResult<Arc<TextureBaseClass>> {
        self.ensure_file_open()?;

        let file = self
            .file
            .as_mut()
            .expect("cache file open after ensure_file_open");

        file.seek(SeekFrom::End(0))?;
        write_u32(file, name.len() as u32)?;
        file.write_all(name.as_bytes())?;
        write_block_header(file, &header)?;
        for offset in &offsets {
            write_u64(file, offset.relative_offset)?;
            write_u64(file, offset.size)?;
        }
        let data_offset = file.stream_position()?;
        file.write_all(&data)?;
        file.flush()?;

        let entry = TextureCacheEntry {
            header: header.clone(),
            offsets,
            data_offset,
            data_size: data.len() as u64,
            cached_surfaces: vec![None; header.num_mipmaps.max(1) as usize],
            texture: None,
            ref_count: AtomicU32::new(0),
            last_access_time: {
                let sync = WW3D::sync_time() as u64;
                if sync > 0 {
                    UNIX_EPOCH + Duration::from_millis(sync)
                } else {
                    SystemTime::now()
                }
            },
            file_time: header.file_time,
        };

        let texture_arc = Arc::new(texture);
        let memory = Self::estimate_texture_memory(texture_arc.as_ref());

        let key = Self::key_for(name);
        if let Some(previous) = self.entries.insert(key.clone(), entry) {
            if let Some(old_texture) = previous.texture {
                self.total_memory_usage = self
                    .total_memory_usage
                    .saturating_sub(Self::estimate_texture_memory(old_texture.as_ref()));
            }
        }

        self.total_memory_usage = self.total_memory_usage.saturating_add(memory);
        if let Some(entry) = self.entries.get_mut(&key) {
            entry.texture = Some(texture_arc.clone());
            entry.ref_count.store(1, Ordering::SeqCst);
        }

        Ok(texture_arc)
    }

    /// Validate that a texture exists in the cache, rebuilding it from disk if necessary.
    pub fn validate_texture(&mut self, texturename: &str) -> bool {
        if self.ensure_file_open().is_err() {
            return false;
        }
        let key = Self::key_for(texturename);

        let Some(source_path) = self.find_source_path(texturename) else {
            return false;
        };

        let file_time = file_time_seconds(&source_path).unwrap_or(0);

        if let Some(entry) = self.entries.get(&key) {
            if entry.file_time == file_time {
                return true;
            }
        }

        // Rebuild the entry by decoding from disk.
        let decoded = match decode_texture_file(&source_path) {
            Ok(data) => data,
            Err(_) => return false,
        };

        let asset_type = kind_to_asset(decoded.kind);
        let mut texture = TextureBaseClass::new(
            decoded.width,
            decoded.height,
            decoded.mip_levels,
            PoolType::Managed,
            asset_type,
        );
        texture.set_name(texturename);
        texture.set_full_path(&source_path.to_string_lossy());
        texture.apply_texture_data(&decoded);

        let (header, offsets, serialized) =
            self.prepare_serialization(texturename, &texture, file_time);

        self.add_entry(texturename, texture, header, offsets, serialized)
            .is_ok()
    }

    /// C++ parity: reset cache file and rewrite version header.
    pub fn reset_file(&mut self) {
        if self.ensure_file_open().is_err() {
            return;
        }
        if let Some(file) = self.file.as_mut() {
            if Self::reset_file_internal(file).is_ok() {
                self.entries.clear();
                self.total_memory_usage = 0;
                self.current_texture = None;
            }
        }
    }

    /// C++ parity: check if texture tag exists in cache.
    pub fn texture_exists(&mut self, fname: &str) -> bool {
        if self.ensure_file_open().is_err() {
            return false;
        }
        self.entries.contains_key(&Self::key_for(fname))
    }

    fn open_texture_handle(&mut self, fname: &str) -> bool {
        if let Some(current) = &self.current_texture {
            if current.eq_ignore_ascii_case(fname) {
                return self.entries.contains_key(&Self::key_for(fname));
            }
            self.close_texture_handle();
        }
        if !self.validate_texture(fname) {
            return false;
        }
        if self.entries.contains_key(&Self::key_for(fname)) {
            self.current_texture = Some(fname.to_string());
            true
        } else {
            false
        }
    }

    fn close_texture_handle(&mut self) {
        self.current_texture = None;
    }

    /// C++ parity: create source-sized surface shell using original dimensions.
    pub fn load_original_texture_surface(&mut self, texturename: &str) -> Option<SrColorSurfaceIFace> {
        if !self.open_texture_handle(texturename) {
            return None;
        }
        let key = Self::key_for(texturename);
        let entry = self.entries.get(&key)?;
        let bytes_per_pixel = approximate_bpp(entry.header.source_format).ceil().max(1.0) as usize;
        let size = (entry.header.source_width.max(1) as usize)
            * (entry.header.source_height.max(1) as usize)
            * bytes_per_pixel;
        Some(SrColorSurfaceIFace::new(
            entry.header.source_width.max(1),
            entry.header.source_height.max(1),
            entry.header.source_format,
            vec![0u8; size.max(1)],
        ))
    }

    /// C++ parity: save an explicit mip chain into cache.
    pub fn save_texture(
        &mut self,
        texturename: &str,
        mreq: &TextureMultiRequest,
        origsurface: &SrColorSurfaceIFace,
    ) -> bool {
        if self.ensure_file_open().is_err() {
            return false;
        }
        if mreq.small_lod < mreq.large_lod {
            return false;
        }

        let mut levels = Vec::new();
        for lod in mreq.large_lod..=mreq.small_lod {
            let Some(surface) = mreq.levels.get(lod as usize).and_then(|s| s.clone()) else {
                return false;
            };
            levels.push(surface);
        }
        if levels.is_empty() {
            return false;
        }

        let source_path = self.find_source_path(texturename);
        let file_time = source_path
            .as_deref()
            .and_then(file_time_seconds)
            .unwrap_or(0);

        let largest = &levels[0];
        let header = TextureBlockHeader {
            file_time,
            num_mipmaps: levels.len() as u32,
            largest_width: largest.width.max(1),
            largest_height: largest.height.max(1),
            source_width: origsurface.width.max(1),
            source_height: origsurface.height.max(1),
            source_format: origsurface.format,
            stored_format: largest.format,
            asset_type: TexAssetType::Regular,
        };

        let mut serialized = Vec::new();
        let mut offsets = Vec::with_capacity(levels.len());
        for level in &levels {
            let relative_offset = serialized.len() as u64;
            let compressed = level.bytes.clone(); // C++ parity: compressor is identity.
            serialized.extend_from_slice(&compressed);
            offsets.push(MipOffset {
                relative_offset,
                size: compressed.len() as u64,
            });
        }

        let mut texture = TextureBaseClass::new(
            header.largest_width,
            header.largest_height,
            header.num_mipmaps,
            PoolType::Managed,
            header.asset_type,
        );
        texture.set_name(texturename);
        texture.set_format(header.stored_format);
        texture.set_dimensions(
            header.largest_width,
            header.largest_height,
            1,
            header.num_mipmaps,
        );
        texture.set_system_memory(serialized.clone());
        let mut mip_layout = Vec::with_capacity(levels.len());
        let mut cursor = 0usize;
        for level in &levels {
            let size = level.bytes.len();
            mip_layout.push(crate::rendering::texture_system::texture_base::SystemMipLevel {
                offset: cursor,
                size,
                width: level.width.max(1),
                height: level.height.max(1),
                depth_or_layers: 1,
                slice_stride: size,
            });
            cursor += size;
        }
        texture.set_system_mip_levels(mip_layout);

        self.add_entry(texturename, texture, header.clone(), offsets, serialized)
            .is_ok()
    }

    /// C++ parity: load texture data into requested mip surfaces.
    pub fn load_texture(&mut self, texturename: &str, mreq: &mut TextureMultiRequest) -> bool {
        if !self.open_texture_handle(texturename) {
            return false;
        }
        if mreq.small_lod < mreq.large_lod {
            return false;
        }

        let key = Self::key_for(texturename);
        let Some(entry) = self.entries.get(&key) else {
            return false;
        };
        let num_mips = entry.header.num_mipmaps as usize;
        let mip_sizes: Vec<usize> = entry.offsets.iter().map(|o| o.size as usize).collect();
        if num_mips == 0 {
            return false;
        }

        let mut idx_size = mip_sizes.first().copied().unwrap_or(0);
        let mut lod = mreq.large_lod as usize;
        let mut lod_size = usize::MAX;

        while lod <= mreq.small_lod as usize {
            let Some(level) = mreq.levels.get(lod).and_then(|s| s.as_ref()) else {
                return false;
            };
            lod_size = level.data_size();
            if lod_size <= idx_size {
                break;
            }
            lod += 1;
        }
        if lod_size == usize::MAX {
            return false;
        }

        let mut idx = 0usize;
        while idx < num_mips {
            idx_size = *mip_sizes.get(idx).unwrap_or(&0);
            if idx_size <= lod_size {
                break;
            }
            idx += 1;
        }

        let first_lod = lod;
        if idx < num_mips && idx_size == lod_size {
            while lod <= mreq.small_lod as usize && idx < num_mips {
                let Some(dest) = mreq.levels.get_mut(lod).and_then(|s| s.as_mut()) else {
                    return false;
                };
                if let Some(src) = self.get_surface(texturename, idx as u32) {
                    dest.copy_from(&src);
                } else {
                    return false;
                }
                idx += 1;
                lod += 1;
            }
        }

        let last_lod = lod.saturating_sub(1);
        let mut working_surface = if first_lod < last_lod {
            mreq.levels
                .get(first_lod)
                .and_then(|s| s.as_ref())
                .cloned()
        } else {
            None
        };

        if (mreq.large_lod as usize) < first_lod {
            if working_surface.is_none() {
                working_surface = self.create_first_texture_as_surface(texturename, mreq.large_lod as usize);
            }
            if let Some(surface) = working_surface.clone() {
                for target_lod in mreq.large_lod as usize..=first_lod {
                    if let Some(dest) = mreq.levels.get_mut(target_lod).and_then(|s| s.as_mut()) {
                        dest.copy_from(&surface);
                    }
                }
            }
        }

        if last_lod < mreq.small_lod as usize {
            if working_surface.is_none() {
                working_surface = self.create_first_texture_as_surface(texturename, mreq.large_lod as usize);
            }
            if let Some(surface) = working_surface.clone() {
                for target_lod in (last_lod + 1)..=mreq.small_lod as usize {
                    if let Some(dest) = mreq.levels.get_mut(target_lod).and_then(|s| s.as_mut()) {
                        dest.copy_from(&surface);
                    }
                }
            }
        }

        true
    }

    fn create_first_texture_as_surface(
        &mut self,
        texturename: &str,
        _requested_lod: usize,
    ) -> Option<SrColorSurfaceIFace> {
        self.get_surface(texturename, 0)
    }

    fn prepare_serialization(
        &self,
        _name: &str,
        texture: &TextureBaseClass,
        file_time: u64,
    ) -> (TextureBlockHeader, Vec<MipOffset>, Vec<u8>) {
        let num_mips = texture.mip_level_count.max(1);

        let header = TextureBlockHeader {
            file_time,
            num_mipmaps: num_mips,
            largest_width: texture.width.max(1),
            largest_height: texture.height.max(1),
            source_width: texture.width.max(1),
            source_height: texture.height.max(1),
            source_format: texture.ww3d_format,
            stored_format: texture.ww3d_format,
            asset_type: texture.asset_type,
        };
        let mut serialized = Vec::new();
        let mut offsets = Vec::new();
        for mip in &texture.system_mip_levels {
            let slice_start = mip.offset;
            let slice_stride = if mip.slice_stride == 0 {
                mip.size
            } else {
                mip.slice_stride
            };
            let slice_end = slice_start + slice_stride;
            let data = &texture.system_memory[slice_start..slice_end];
            let relative_offset = serialized.len() as u64;
            serialized.extend_from_slice(data);
            offsets.push(MipOffset {
                relative_offset,
                size: slice_stride as u64,
            });
        }

        if offsets.len() != num_mips as usize {
            // Some assets only provide the top level; infer others by halving while clamping.
            let mut inferred_offsets = Vec::with_capacity(num_mips as usize);
            let mut cursor = 0u64;
            for level in 0..num_mips {
                let width = (header.largest_width >> level).max(1);
                let height = (header.largest_height >> level).max(1);
                let bytes_per_pixel = approximate_bpp(texture.ww3d_format);
                let size = (width as f64 * height as f64 * bytes_per_pixel) as u64;
                inferred_offsets.push(MipOffset {
                    relative_offset: cursor,
                    size,
                });
                cursor += size;
            }
            offsets = inferred_offsets;
            serialized.resize(cursor as usize, 0);
        }

        (header, offsets, serialized)
    }

    /// Retrieve a texture from the cache. If the entry is stored on disk only it will be
    /// reconstructed and kept resident until released.
    pub fn get_texture(&mut self, fname: &str) -> Option<Arc<TextureBaseClass>> {
        if !self.validate_texture(fname) {
            return None;
        }
        let key = Self::key_for(fname);

        let load_request = {
            let entry = self.entries.get_mut(&key)?;
            entry.add_ref();
            entry.touch();
            if let Some(texture) = entry.texture.clone() {
                return Some(texture);
            }
            (
                entry.header.clone(),
                entry.offsets.clone(),
                entry.data_offset,
                entry.data_size,
            )
        };

        let (header, offsets, data_offset, data_size) = load_request;
        let bytes = self.read_bytes(data_offset, data_size).ok()?;
        let restored = restore_texture_from_metadata(fname, &header, &offsets, bytes).ok()?;
        let texture = Arc::new(restored);
        let memory = Self::estimate_texture_memory(texture.as_ref());

        let entry = self.entries.get_mut(&key)?;
        if entry.texture.is_none() {
            entry.texture = Some(texture.clone());
            self.total_memory_usage = self.total_memory_usage.saturating_add(memory);
        }
        Some(texture)
    }

    /// Add a fully realised texture to the cache.
    pub fn add_texture(
        &mut self,
        fname: &str,
        texture: TextureBaseClass,
    ) -> RendererResult<Arc<TextureBaseClass>> {
        let file_time = file_time_seconds(Path::new(fname)).unwrap_or(0);
        let (header, offsets, data) = self.prepare_serialization(fname, &texture, file_time);
        self.add_entry(fname, texture, header, offsets, data)
    }

    /// Release a texture reference. When the reference count hits zero the resident copy is dropped.
    pub fn release_texture(&mut self, fname: &str) {
        let key = Self::key_for(fname);
        if let Some(entry) = self.entries.get_mut(&key) {
            if entry.release_ref() == 0 {
                if let Some(texture) = entry.drop_texture() {
                    self.total_memory_usage = self
                        .total_memory_usage
                        .saturating_sub(Self::estimate_texture_memory(texture.as_ref()));
                }
            }
        }
    }

    /// Retrieve a mip-map surface from the cache.
    pub fn get_surface(
        &mut self,
        texturename: &str,
        reduce_factor: u32,
    ) -> Option<SrColorSurfaceIFace> {
        if !self.validate_texture(texturename) {
            return None;
        }
        let key = Self::key_for(texturename);

        let load_request = {
            let entry = self.entries.get_mut(&key)?;
            let level_index = (reduce_factor as usize).min(entry.offsets.len().saturating_sub(1));
            if let Some(surface) = entry.cached_surfaces[level_index].clone() {
                entry.touch();
                return Some(surface);
            }
            entry.touch();
            (
                entry.header.clone(),
                entry.offsets[level_index].clone(),
                entry.data_offset,
                level_index,
            )
        };

        let (header, offset, data_offset, level_index) = load_request;
        let width = level_dimension(header.largest_width, level_index);
        let height = level_dimension(header.largest_height, level_index);
        let bytes = self
            .read_bytes(data_offset + offset.relative_offset, offset.size)
            .ok()?;

        let surface = SrColorSurfaceIFace::new(width, height, header.stored_format, bytes);
        let entry = self.entries.get_mut(&key)?;
        entry.cached_surfaces[level_index] = Some(surface.clone());
        Some(surface)
    }

    /// Release textures that are no longer referenced.
    pub fn cleanup_unused_textures(&mut self) {
        let timeout = Duration::from_secs_f64(self.config.unused_timeout_seconds);
        let now = SystemTime::now();
        let mut reclaimed = 0u64;

        for entry in self.entries.values_mut() {
            if entry.is_unused() {
                if let Ok(elapsed) = now.duration_since(entry.last_access_time) {
                    if elapsed >= timeout {
                        if let Some(texture) = entry.drop_texture() {
                            reclaimed = reclaimed
                                .saturating_add(Self::estimate_texture_memory(texture.as_ref()));
                        }
                        entry.cached_surfaces.fill(None);
                    }
                }
            }
        }

        self.total_memory_usage = self.total_memory_usage.saturating_sub(reclaimed);
    }

    /// Force cache maintenance.
    pub fn garbage_collect(&mut self) {
        self.cleanup_unused_textures();
        self.enforce_limits();
    }

    fn enforce_limits(&mut self) {
        let max_bytes = self.config.max_memory_mb * 1024 * 1024;
        if self.total_memory_usage <= max_bytes {
            return;
        }

        // Evict least recently used entries until we fall below the limit.
        let mut entries: Vec<_> = self.entries.iter_mut().collect();
        entries.sort_by_key(|(_, entry)| entry.last_access_time);

        let mut current_usage = self.total_memory_usage;
        for (_, entry) in entries {
            if current_usage <= max_bytes {
                break;
            }
            if entry.is_unused() {
                if let Some(texture) = entry.drop_texture() {
                    current_usage = current_usage
                        .saturating_sub(Self::estimate_texture_memory(texture.as_ref()));
                }
                entry.cached_surfaces.fill(None);
            }
        }

        self.total_memory_usage = current_usage;
    }

    /// Return cache statistics.
    pub fn get_cache_stats(&self) -> TextureCacheStats {
        let used = self
            .entries
            .values()
            .filter(|entry| entry.texture.is_some())
            .count();
        let unused = self.entries.len().saturating_sub(used);

        TextureCacheStats {
            total_entries: self.entries.len(),
            used_entries: used,
            unused_entries: unused,
            total_memory_mb: self.total_memory_usage / (1024 * 1024),
            max_memory_mb: self.config.max_memory_mb,
        }
    }

    /// Drop every cached entry from memory and disk.
    pub fn clear_cache(&mut self) {
        self.entries.clear();
        self.total_memory_usage = 0;
        self.current_texture = None;
        if let Some(ref mut file) = self.file {
            file.set_len(0).ok();
            file.seek(SeekFrom::Start(0)).ok();
            file.write_all(&CACHE_MAGIC).ok();
            write_u32(file, CACHE_VERSION).ok();
            file.flush().ok();
        }
    }

    /// Re-open the cache file for direct access.
    pub fn open_cache_file(&mut self) -> bool {
        self.ensure_file_open().is_ok()
    }

    /// Close the cache file handle.
    pub fn close_cache_file(&mut self) {
        if let Some(file) = &mut self.file {
            let _ = file.flush();
        }
        self.file = None;
        self.current_texture = None;
    }

    /// Persisted cache path.
    pub fn get_cache_file_path(&self) -> String {
        self.cache_path().to_string_lossy().into_owned()
    }

    /// Number of entries recorded in the cache.
    pub fn get_num_cached_textures(&self) -> usize {
        self.entries.len()
    }

    /// Total on-disk cache size.
    pub fn get_cache_file_size(&self) -> u64 {
        std::fs::metadata(self.cache_path())
            .map(|meta| meta.len())
            .unwrap_or(0)
    }

    /// Whether the cache is open.
    pub fn is_cache_valid(&self) -> bool {
        self.file.is_some()
    }

    fn read_bytes(&self, offset: u64, size: u64) -> RendererResult<Vec<u8>> {
        let mut file = File::open(self.cache_path())?;
        file.seek(SeekFrom::Start(offset))?;
        let mut bytes = vec![0u8; size as usize];
        file.read_exact(&mut bytes)?;
        Ok(bytes)
    }

    fn estimate_texture_memory(texture: &TextureBaseClass) -> u64 {
        let mut bytes_per_pixel = approximate_bpp(texture.ww3d_format);
        if texture.mip_level_count > 1 {
            bytes_per_pixel *= 1.33;
        }
        let pixels = texture.width.max(1) as f64
            * texture.height.max(1) as f64
            * texture.depth.max(1) as f64;
        (pixels * bytes_per_pixel) as u64
    }
}

impl Drop for TextureFileCache {
    fn drop(&mut self) {
        self.close_cache_file();
    }
}

/// Cache statistics for diagnostics.
#[derive(Debug, Clone)]
pub struct TextureCacheStats {
    pub total_entries: usize,
    pub used_entries: usize,
    pub unused_entries: usize,
    pub total_memory_mb: u64,
    pub max_memory_mb: u64,
}

impl TextureCacheStats {
    pub fn memory_usage_percent(&self) -> f32 {
        if self.max_memory_mb == 0 {
            0.0
        } else {
            (self.total_memory_mb as f32 / self.max_memory_mb as f32) * 100.0
        }
    }

    pub fn is_memory_critical(&self) -> bool {
        self.memory_usage_percent() > 90.0
    }
}

fn level_dimension(base: u32, level: usize) -> u32 {
    (base >> level).max(1)
}

fn restore_texture_from_metadata(
    name: &str,
    header: &TextureBlockHeader,
    offsets: &[MipOffset],
    data: Vec<u8>,
) -> RendererResult<TextureBaseClass> {
    let kind = asset_type_to_kind(header.asset_type);

    let mut mip_layout = Vec::with_capacity(offsets.len());
    for (level, offset) in offsets.iter().enumerate() {
        mip_layout.push(TextureMipLevel {
            offset: offset.relative_offset as usize,
            size: offset.size as usize,
            width: level_dimension(header.largest_width, level),
            height: level_dimension(header.largest_height, level),
            depth_or_layers: 1,
            slice_stride: offset.size as usize,
        });
    }

    let texture_data = TextureData {
        width: header.largest_width,
        height: header.largest_height,
        depth: 1,
        mip_levels: header.num_mipmaps,
        format: header.stored_format,
        kind,
        data,
        mip_layout,
        format_decision: None,
    };

    let mut texture = TextureBaseClass::new(
        header.largest_width,
        header.largest_height,
        header.num_mipmaps,
        PoolType::Managed,
        header.asset_type,
    );
    texture.set_name(name);
    texture.apply_texture_data(&texture_data);
    Ok(texture)
}

fn approximate_bpp(format: WW3DFormat) -> f64 {
    match format {
        WW3DFormat::DXT1 => 0.5,
        WW3DFormat::DXT2 | WW3DFormat::DXT3 | WW3DFormat::DXT4 | WW3DFormat::DXT5 => 1.0,
        WW3DFormat::R5G6B5 | WW3DFormat::X1R5G5B5 | WW3DFormat::A1R5G5B5 | WW3DFormat::A4R4G4B4 => {
            2.0
        }
        _ => 4.0,
    }
}

fn asset_type_to_kind(asset: TexAssetType) -> TextureDataKind {
    match asset {
        TexAssetType::Regular => TextureDataKind::Texture2D,
        TexAssetType::Cubemap => TextureDataKind::CubeMap,
        TexAssetType::Volume => TextureDataKind::Volume,
    }
}

fn kind_to_asset(kind: TextureDataKind) -> TexAssetType {
    match kind {
        TextureDataKind::Texture2D => TexAssetType::Regular,
        TextureDataKind::CubeMap => TexAssetType::Cubemap,
        TextureDataKind::Volume => TexAssetType::Volume,
    }
}

fn tex_asset_type_to_u32(asset: TexAssetType) -> u32 {
    match asset {
        TexAssetType::Regular => 0,
        TexAssetType::Cubemap => 1,
        TexAssetType::Volume => 2,
    }
}

fn tex_asset_type_from_u32(value: u32) -> TexAssetType {
    match value {
        1 => TexAssetType::Cubemap,
        2 => TexAssetType::Volume,
        _ => TexAssetType::Regular,
    }
}

fn ww3d_format_to_u32(format: WW3DFormat) -> u32 {
    match format {
        WW3DFormat::Unknown => 0,
        WW3DFormat::R8G8B8 => 1,
        WW3DFormat::A8R8G8B8 => 2,
        WW3DFormat::X8R8G8B8 => 3,
        WW3DFormat::R8G8B8A8 => 4,
        WW3DFormat::R5G6B5 => 5,
        WW3DFormat::X1R5G5B5 => 6,
        WW3DFormat::A1R5G5B5 => 7,
        WW3DFormat::A4R4G4B4 => 8,
        WW3DFormat::R3G3B2 => 9,
        WW3DFormat::A8 => 10,
        WW3DFormat::A8R3G3B2 => 11,
        WW3DFormat::X4R4G4B4 => 12,
        WW3DFormat::A8P8 => 13,
        WW3DFormat::P8 => 14,
        WW3DFormat::L8 => 15,
        WW3DFormat::A8L8 => 16,
        WW3DFormat::A4L4 => 17,
        WW3DFormat::U8V8 => 18,
        WW3DFormat::L6V5U5 => 19,
        WW3DFormat::X8L8V8U8 => 20,
        WW3DFormat::DXT1 => 21,
        WW3DFormat::DXT2 => 22,
        WW3DFormat::DXT3 => 23,
        WW3DFormat::DXT4 => 24,
        WW3DFormat::DXT5 => 25,
        WW3DFormat::D16 => 26,
        WW3DFormat::D24S8 => 27,
        WW3DFormat::D32 => 28,
        WW3DFormat::D16Lockable => 29,
    }
}

fn ww3d_format_from_u32(value: u32) -> Option<WW3DFormat> {
    Some(match value {
        0 => WW3DFormat::Unknown,
        1 => WW3DFormat::R8G8B8,
        2 => WW3DFormat::A8R8G8B8,
        3 => WW3DFormat::X8R8G8B8,
        4 => WW3DFormat::R8G8B8A8,
        5 => WW3DFormat::R5G6B5,
        6 => WW3DFormat::X1R5G5B5,
        7 => WW3DFormat::A1R5G5B5,
        8 => WW3DFormat::A4R4G4B4,
        9 => WW3DFormat::R3G3B2,
        10 => WW3DFormat::A8,
        11 => WW3DFormat::A8R3G3B2,
        12 => WW3DFormat::X4R4G4B4,
        13 => WW3DFormat::A8P8,
        14 => WW3DFormat::P8,
        15 => WW3DFormat::L8,
        16 => WW3DFormat::A8L8,
        17 => WW3DFormat::A4L4,
        18 => WW3DFormat::U8V8,
        19 => WW3DFormat::L6V5U5,
        20 => WW3DFormat::X8L8V8U8,
        21 => WW3DFormat::DXT1,
        22 => WW3DFormat::DXT2,
        23 => WW3DFormat::DXT3,
        24 => WW3DFormat::DXT4,
        25 => WW3DFormat::DXT5,
        26 => WW3DFormat::D16,
        27 => WW3DFormat::D24S8,
        28 => WW3DFormat::D32,
        29 => WW3DFormat::D16Lockable,
        _ => return None,
    })
}

fn write_block_header(file: &mut File, header: &TextureBlockHeader) -> std::io::Result<()> {
    write_u64(file, header.file_time)?;
    write_u32(file, header.num_mipmaps)?;
    write_u32(file, header.largest_width)?;
    write_u32(file, header.largest_height)?;
    write_u32(file, header.source_width)?;
    write_u32(file, header.source_height)?;
    write_u32(file, ww3d_format_to_u32(header.source_format))?;
    write_u32(file, ww3d_format_to_u32(header.stored_format))?;
    write_u32(file, tex_asset_type_to_u32(header.asset_type))?;
    Ok(())
}

fn read_block_header(file: &mut File) -> RendererResult<TextureBlockHeader> {
    let file_time = read_u64(file)?;
    let num_mipmaps = read_u32(file)?;
    let largest_width = read_u32(file)?;
    let largest_height = read_u32(file)?;
    let source_width = read_u32(file)?;
    let source_height = read_u32(file)?;
    let source_format = ww3d_format_from_u32(read_u32(file)?)
        .ok_or_else(|| Error::InvalidData("unknown source format".into()))?;
    let stored_format = ww3d_format_from_u32(read_u32(file)?)
        .ok_or_else(|| Error::InvalidData("unknown stored format".into()))?;
    let asset_type = tex_asset_type_from_u32(read_u32(file)?);

    Ok(TextureBlockHeader {
        file_time,
        num_mipmaps,
        largest_width,
        largest_height,
        source_width,
        source_height,
        source_format,
        stored_format,
        asset_type,
    })
}

fn write_u32(file: &mut File, value: u32) -> std::io::Result<()> {
    file.write_all(&value.to_le_bytes())
}

fn write_u64(file: &mut File, value: u64) -> std::io::Result<()> {
    file.write_all(&value.to_le_bytes())
}

fn read_u32(file: &mut File) -> std::io::Result<u32> {
    let mut buf = [0u8; 4];
    file.read_exact(&mut buf)?;
    Ok(u32::from_le_bytes(buf))
}

fn read_u32_optional(file: &mut File) -> std::io::Result<Option<u32>> {
    let mut buf = [0u8; 4];
    match file.read(&mut buf) {
        Ok(0) => Ok(None),
        Ok(4) => Ok(Some(u32::from_le_bytes(buf))),
        Ok(_) => Err(std::io::Error::new(
            ErrorKind::UnexpectedEof,
            "unexpected EOF while reading u32",
        )),
        Err(err) => Err(err),
    }
}

fn read_u64(file: &mut File) -> std::io::Result<u64> {
    let mut buf = [0u8; 8];
    file.read_exact(&mut buf)?;
    Ok(u64::from_le_bytes(buf))
}

fn file_time_seconds(path: &Path) -> Option<u64> {
    let metadata = std::fs::metadata(path).ok()?;
    let modified = metadata.modified().ok()?;
    modified
        .duration_since(UNIX_EPOCH)
        .ok()
        .map(|duration| duration.as_secs())
}

fn texture_cache_slot() -> &'static Mutex<Option<TextureFileCache>> {
    static STORAGE: OnceLock<Mutex<Option<TextureFileCache>>> = OnceLock::new();
    STORAGE.get_or_init(|| Mutex::new(None))
}

fn lock_texture_cache_slot() -> MutexGuard<'static, Option<TextureFileCache>> {
    match texture_cache_slot().lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    }
}

/// Scoped handle to the global cache.
pub struct TextureCacheHandle<'a> {
    guard: MutexGuard<'a, Option<TextureFileCache>>,
}

impl<'a> Deref for TextureCacheHandle<'a> {
    type Target = TextureFileCache;

    fn deref(&self) -> &Self::Target {
        self.guard
            .as_ref()
            .expect("texture cache must be initialised before use")
    }
}

impl<'a> DerefMut for TextureCacheHandle<'a> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.guard
            .as_mut()
            .expect("texture cache must be initialised before use")
    }
}

/// Initialise the global texture cache.
pub fn init_global_texture_cache(file_prefix: &str) {
    let mut guard = lock_texture_cache_slot();
    *guard = Some(TextureFileCache::new(file_prefix));
}

/// Borrow the global cache.
pub fn get_global_texture_cache() -> Option<TextureCacheHandle<'static>> {
    let guard = lock_texture_cache_slot();
    if guard.is_none() {
        None
    } else {
        Some(TextureCacheHandle { guard })
    }
}

/// Tear down the global cache.
pub fn shutdown_global_texture_cache() {
    let mut guard = lock_texture_cache_slot();
    *guard = None;
}
