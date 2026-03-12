use crate::rendering::texture_system::texture_base::TextureClass;
use std::collections::HashMap;
use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::Path;
use std::sync::{Arc, Mutex, OnceLock};

// Constants
pub const THUMB_FILE_HEADER: &[u8; 4] = b"THU6";
pub const THUMBNAIL_SIZE: usize = 64;
const THUMB_COMPRESS_RAW: &[u8; 4] = b"RAW0";
const THUMB_COMPRESS_RLE: &[u8; 4] = b"RLE1";

/// Thumbnail data structure
pub struct ThumbnailClass {
    manager: Arc<Mutex<ThumbnailManagerClass>>,
    name: String,
    bitmap: Option<Vec<u8>>,
    allocated: bool,
    width: u32,
    height: u32,
    original_texture_width: u32,
    original_texture_height: u32,
    original_texture_mip_level_count: u32,
    original_texture_format: u32, // Stored WW3DFormat discriminator
    date_time: u32,
}

impl ThumbnailClass {
    /// Create new thumbnail
    pub fn new(
        manager: Arc<Mutex<ThumbnailManagerClass>>,
        name: &str,
        bitmap: Option<Vec<u8>>,
        width: u32,
        height: u32,
        original_width: u32,
        original_height: u32,
        original_mip_levels: u32,
        original_format: u32,
        allocated: bool,
        date_time: u32,
    ) -> Self {
        let thumbnail = Self {
            manager: manager.clone(),
            name: name.to_string(),
            bitmap,
            allocated,
            width,
            height,
            original_texture_width: original_width,
            original_texture_height: original_height,
            original_texture_mip_level_count: original_mip_levels,
            original_texture_format: original_format,
            date_time,
        };

        // Add to manager's hash
        if let Ok(mut mgr) = manager.lock() {
            mgr.insert_to_hash(&thumbnail);
        }

        thumbnail
    }

    /// Get thumbnail name
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Get bitmap data
    pub fn bitmap(&self) -> Option<&[u8]> {
        self.bitmap.as_ref().map(|v| v.as_slice())
    }

    /// Get width
    pub fn width(&self) -> u32 {
        self.width
    }

    /// Get height
    pub fn height(&self) -> u32 {
        self.height
    }

    /// Get original texture width
    pub fn original_texture_width(&self) -> u32 {
        self.original_texture_width
    }

    /// Get original texture height
    pub fn original_texture_height(&self) -> u32 {
        self.original_texture_height
    }

    /// Get original texture mip level count
    pub fn original_texture_mip_level_count(&self) -> u32 {
        self.original_texture_mip_level_count
    }

    /// Get original texture format
    pub fn original_texture_format(&self) -> u32 {
        self.original_texture_format
    }

    /// Get date/time
    pub fn date_time(&self) -> u32 {
        self.date_time
    }

    /// Check if bitmap is allocated
    pub fn is_allocated(&self) -> bool {
        self.allocated
    }
}

/// Thumbnail manager class
pub struct ThumbnailManagerClass {
    thumbnails: HashMap<String, Arc<ThumbnailClass>>,
    create_thumbnail_if_not_found: bool,
}

impl ThumbnailManagerClass {
    /// Create new thumbnail manager
    pub fn new() -> Arc<Mutex<Self>> {
        Arc::new(Mutex::new(Self {
            thumbnails: HashMap::new(),
            create_thumbnail_if_not_found: false,
        }))
    }

    /// Insert thumbnail into hash
    pub fn insert_to_hash(&mut self, thumbnail: &ThumbnailClass) {
        self.thumbnails.insert(
            thumbnail.name().to_string(),
            Arc::new(ThumbnailClass {
                manager: thumbnail.manager.clone(),
                name: thumbnail.name.clone(),
                bitmap: thumbnail.bitmap.clone(),
                allocated: thumbnail.allocated,
                width: thumbnail.width,
                height: thumbnail.height,
                original_texture_width: thumbnail.original_texture_width,
                original_texture_height: thumbnail.original_texture_height,
                original_texture_mip_level_count: thumbnail.original_texture_mip_level_count,
                original_texture_format: thumbnail.original_texture_format,
                date_time: thumbnail.date_time,
            }),
        );
    }

    /// Get thumbnail by name
    pub fn get_thumbnail(&self, name: &str) -> Option<&Arc<ThumbnailClass>> {
        self.thumbnails.get(name)
    }

    /// Create thumbnail from texture
    pub fn create_thumbnail(&mut self, texture: &TextureClass) -> Option<Arc<ThumbnailClass>> {
        let name = texture.name().to_string();

        // Generate thumbnail bitmap (simplified - just take top-left corner)
        let thumbnail_bitmap = self.generate_thumbnail_bitmap(texture)?;

        let thumbnail_arc = Arc::new(ThumbnailClass {
            manager: Arc::new(Mutex::new(self.clone())),
            name: name.clone(),
            bitmap: Some(thumbnail_bitmap),
            allocated: true,
            width: THUMBNAIL_SIZE as u32,
            height: THUMBNAIL_SIZE as u32,
            original_texture_width: texture.width(),
            original_texture_height: texture.height(),
            original_texture_mip_level_count: texture.mip_level_count(),
            original_texture_format: texture.ww3d_format as u32,
            date_time: self.get_current_time(),
        });
        self.thumbnails.insert(name, thumbnail_arc.clone());

        Some(thumbnail_arc)
    }

    /// Generate thumbnail bitmap from texture
    fn generate_thumbnail_bitmap(&self, texture: &TextureClass) -> Option<Vec<u8>> {
        let surface = texture.get_surface_level(0)?;
        let source = surface.lock();
        let src_width = texture.width().max(1) as usize;
        let src_height = texture.height().max(1) as usize;
        if source.pixels().is_empty() || source.pitch() == 0 {
            return None;
        }

        let bytes_per_pixel = (texture.ww3d_format.bytes_per_pixel().max(1)) as usize;
        if bytes_per_pixel < 3 {
            return None;
        }

        let mut output = vec![0u8; THUMBNAIL_SIZE * THUMBNAIL_SIZE * 4];
        for ty in 0..THUMBNAIL_SIZE {
            let sy = (ty * src_height) / THUMBNAIL_SIZE;
            for tx in 0..THUMBNAIL_SIZE {
                let sx = (tx * src_width) / THUMBNAIL_SIZE;
                let src_offset = sy
                    .checked_mul(source.pitch())?
                    .checked_add(sx.checked_mul(bytes_per_pixel)?)?;
                if src_offset + bytes_per_pixel > source.pixels().len() {
                    continue;
                }

                let pixel = &source.pixels()[src_offset..src_offset + bytes_per_pixel];
                let (r, g, b, a) = match bytes_per_pixel {
                    4 => (pixel[0], pixel[1], pixel[2], pixel[3]),
                    3 => (pixel[0], pixel[1], pixel[2], 255),
                    2 => {
                        let value = u16::from_le_bytes([pixel[0], pixel[1]]);
                        let r = ((value >> 11) & 0x1F) as u8;
                        let g = ((value >> 5) & 0x3F) as u8;
                        let b = (value & 0x1F) as u8;
                        (
                            (r << 3) | (r >> 2),
                            (g << 2) | (g >> 4),
                            (b << 3) | (b >> 2),
                            255,
                        )
                    }
                    _ => continue,
                };

                let dst_offset = (ty * THUMBNAIL_SIZE + tx) * 4;
                output[dst_offset..dst_offset + 4].copy_from_slice(&[r, g, b, a]);
            }
        }

        Some(output)
    }

    /// Load thumbnail from file
    pub fn load_thumbnail(&mut self, filename: &str) -> Option<Arc<ThumbnailClass>> {
        let path = Path::new(filename);
        let mut file = File::open(path).ok()?;
        let mut data = Vec::new();
        file.read_to_end(&mut data).ok()?;
        if data.len() < 40 || &data[0..4] != THUMB_FILE_HEADER {
            return None;
        }

        let mut offset = 4usize;
        let read_u32 = |bytes: &[u8], at: &mut usize| -> Option<u32> {
            let end = at.checked_add(4)?;
            if end > bytes.len() {
                return None;
            }
            let value = u32::from_le_bytes(bytes[*at..end].try_into().ok()?);
            *at = end;
            Some(value)
        };

        let width = read_u32(&data, &mut offset)?;
        let height = read_u32(&data, &mut offset)?;
        let original_width = read_u32(&data, &mut offset)?;
        let original_height = read_u32(&data, &mut offset)?;
        let original_mips = read_u32(&data, &mut offset)?;
        let original_format = read_u32(&data, &mut offset)?;
        let date_time = read_u32(&data, &mut offset)?;
        let bitmap_len = read_u32(&data, &mut offset)? as usize;

        let bitmap_end = offset.checked_add(bitmap_len)?;
        if bitmap_end > data.len() {
            return None;
        }
        let bitmap = decompress_thumbnail_data(&data[offset..bitmap_end]);
        let expected_bitmap_len = width
            .checked_mul(height)
            .and_then(|pixels| pixels.checked_mul(4))
            .map(|bytes| bytes as usize)?;
        if bitmap.len() != expected_bitmap_len {
            return None;
        }

        let name = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or(filename)
            .to_string();
        let thumbnail = Arc::new(ThumbnailClass {
            manager: Arc::new(Mutex::new(self.clone())),
            name: name.clone(),
            bitmap: Some(bitmap),
            allocated: true,
            width,
            height,
            original_texture_width: original_width,
            original_texture_height: original_height,
            original_texture_mip_level_count: original_mips,
            original_texture_format: original_format,
            date_time,
        });
        self.thumbnails.insert(name, thumbnail.clone());
        Some(thumbnail)
    }

    /// Save thumbnail to file
    pub fn save_thumbnail(&self, thumbnail: &ThumbnailClass, filename: &str) -> bool {
        let Some(bitmap) = thumbnail.bitmap.as_ref() else {
            return false;
        };

        let mut payload = Vec::new();
        payload.extend_from_slice(THUMB_FILE_HEADER);
        payload.extend_from_slice(&thumbnail.width.to_le_bytes());
        payload.extend_from_slice(&thumbnail.height.to_le_bytes());
        payload.extend_from_slice(&thumbnail.original_texture_width.to_le_bytes());
        payload.extend_from_slice(&thumbnail.original_texture_height.to_le_bytes());
        payload.extend_from_slice(&thumbnail.original_texture_mip_level_count.to_le_bytes());
        payload.extend_from_slice(&thumbnail.original_texture_format.to_le_bytes());
        payload.extend_from_slice(&thumbnail.date_time.to_le_bytes());

        let compressed = compress_thumbnail_data(bitmap);
        payload.extend_from_slice(&(compressed.len() as u32).to_le_bytes());
        payload.extend_from_slice(&compressed);

        if let Some(parent) = Path::new(filename).parent() {
            if !parent.as_os_str().is_empty() && fs::create_dir_all(parent).is_err() {
                return false;
            }
        }

        match File::create(filename) {
            Ok(mut file) => file.write_all(&payload).is_ok(),
            Err(_) => false,
        }
    }

    /// Get current time as Unix timestamp seconds.
    fn get_current_time(&self) -> u32 {
        use std::time::{SystemTime, UNIX_EPOCH};
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs() as u32
    }

    /// Set create thumbnail if not found flag
    pub fn set_create_thumbnail_if_not_found(&mut self, create: bool) {
        self.create_thumbnail_if_not_found = create;
    }

    /// Get create thumbnail if not found flag
    pub fn get_create_thumbnail_if_not_found(&self) -> bool {
        self.create_thumbnail_if_not_found
    }

    /// Clear all thumbnails
    pub fn clear(&mut self) {
        self.thumbnails.clear();
    }

    /// Get number of thumbnails
    pub fn count(&self) -> usize {
        self.thumbnails.len()
    }
}

impl Clone for ThumbnailManagerClass {
    fn clone(&self) -> Self {
        Self {
            thumbnails: self.thumbnails.clone(),
            create_thumbnail_if_not_found: self.create_thumbnail_if_not_found,
        }
    }
}

fn thumbnail_manager_cell() -> &'static OnceLock<Arc<Mutex<ThumbnailManagerClass>>> {
    static CELL: OnceLock<Arc<Mutex<ThumbnailManagerClass>>> = OnceLock::new();
    &CELL
}

/// Initialise the global thumbnail manager. Subsequent calls refresh the internal state instead of
/// allocating an additional singleton, keeping legacy entry points working with safe Rust semantics.
pub fn init_global_thumbnail_manager() -> Arc<Mutex<ThumbnailManagerClass>> {
    let manager = ThumbnailManagerClass::new();
    if let Err(existing) = thumbnail_manager_cell().set(manager.clone()) {
        if let Ok(mut guard) = existing.lock() {
            if let Ok(source) = manager.lock() {
                *guard = source.clone();
            } else {
                guard.clear();
            }
        }
        existing.clone()
    } else {
        manager
    }
}

/// Access the global thumbnail manager if it has been initialised.
pub fn get_global_thumbnail_manager() -> Option<Arc<Mutex<ThumbnailManagerClass>>> {
    thumbnail_manager_cell().get().cloned()
}

/// Clear the global thumbnail manager’s contents without tearing down the singleton. This avoids
/// `static mut` lifetime issues while still providing the semantic “shutdown” hook expected by the
/// legacy API.
pub fn shutdown_global_thumbnail_manager() {
    if let Some(manager) = thumbnail_manager_cell().get() {
        if let Ok(mut guard) = manager.lock() {
            guard.clear();
        }
    }
}

/// Create hash name from thumbnail name
pub fn create_hash_name(thumb_name: &str) -> String {
    let mut name = thumb_name.to_string();
    if name.to_lowercase().ends_with(".tga") || name.to_lowercase().ends_with(".dds") {
        let len = name.len();
        name.truncate(len - 4);
    }
    name.to_lowercase()
}

/// Utility functions for thumbnail management

/// Check if thumbnail file exists
pub fn thumbnail_file_exists(filename: &str) -> bool {
    let path = get_thumbnail_file_path(filename);
    Path::new(&path).exists()
}

/// Get thumbnail file path
pub fn get_thumbnail_file_path(texture_name: &str) -> String {
    format!("{}.thu", create_hash_name(texture_name))
}

/// Validate thumbnail cache
pub fn validate_thumbnail_cache() -> bool {
    let entries = match fs::read_dir(".") {
        Ok(entries) => entries,
        Err(_) => return true,
    };

    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("thu") {
            continue;
        }
        let mut file = match File::open(&path) {
            Ok(file) => file,
            Err(_) => return false,
        };
        let mut header = [0u8; 4];
        if file.read_exact(&mut header).is_err() || &header != THUMB_FILE_HEADER {
            return false;
        }
    }

    true
}

/// Clean up old thumbnails
pub fn cleanup_old_thumbnails(max_age_days: u32) {
    let max_age_secs = (max_age_days as u64) * 24 * 60 * 60;
    let now = std::time::SystemTime::now();

    let entries = match fs::read_dir(".") {
        Ok(entries) => entries,
        Err(_) => return,
    };

    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("thu") {
            continue;
        }

        let Ok(metadata) = entry.metadata() else {
            continue;
        };
        let Ok(modified) = metadata.modified() else {
            continue;
        };
        let Ok(age) = now.duration_since(modified) else {
            continue;
        };
        if age.as_secs() > max_age_secs {
            let _ = fs::remove_file(path);
        }
    }
}

/// Get thumbnail cache size
pub fn get_thumbnail_cache_size() -> u64 {
    let entries = match fs::read_dir(".") {
        Ok(entries) => entries,
        Err(_) => return 0,
    };

    entries
        .flatten()
        .filter_map(|entry| {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) == Some("thu") {
                entry.metadata().ok().map(|meta| meta.len())
            } else {
                None
            }
        })
        .sum()
}

/// Compress thumbnail data
pub fn compress_thumbnail_data(data: &[u8]) -> Vec<u8> {
    let mut rle = Vec::with_capacity(data.len() / 2 + 8);
    rle.extend_from_slice(THUMB_COMPRESS_RLE);
    rle.extend_from_slice(&(data.len() as u32).to_le_bytes());

    let mut index = 0usize;
    while index < data.len() {
        let value = data[index];
        let mut run_len: u8 = 1;
        while index + (run_len as usize) < data.len()
            && run_len < u8::MAX
            && data[index + (run_len as usize)] == value
        {
            run_len = run_len.saturating_add(1);
        }
        rle.push(run_len);
        rle.push(value);
        index += run_len as usize;
    }

    let mut raw = Vec::with_capacity(data.len() + 8);
    raw.extend_from_slice(THUMB_COMPRESS_RAW);
    raw.extend_from_slice(&(data.len() as u32).to_le_bytes());
    raw.extend_from_slice(data);

    if rle.len() < raw.len() {
        rle
    } else {
        raw
    }
}

/// Decompress thumbnail data
pub fn decompress_thumbnail_data(data: &[u8]) -> Vec<u8> {
    if data.len() < 8 {
        return data.to_vec();
    }

    let magic = &data[0..4];
    let expected_len = u32::from_le_bytes([data[4], data[5], data[6], data[7]]) as usize;
    let payload = &data[8..];

    if magic == THUMB_COMPRESS_RAW {
        if payload.len() == expected_len {
            return payload.to_vec();
        }
        return Vec::new();
    }

    if magic == THUMB_COMPRESS_RLE {
        let mut output = Vec::with_capacity(expected_len);
        let mut cursor = 0usize;
        while cursor + 1 < payload.len() {
            let run_len = payload[cursor] as usize;
            let value = payload[cursor + 1];
            output.extend(std::iter::repeat_n(value, run_len));
            cursor += 2;
            if output.len() > expected_len {
                return Vec::new();
            }
        }
        if output.len() == expected_len {
            return output;
        }
        return Vec::new();
    }

    // Legacy uncompressed payload path.
    data.to_vec()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn save_and_load_thumbnail_round_trip() {
        let mut manager = ThumbnailManagerClass {
            thumbnails: HashMap::new(),
            create_thumbnail_if_not_found: false,
        };

        let thumbnail = ThumbnailClass {
            manager: Arc::new(Mutex::new(manager.clone())),
            name: "test_thumb".to_string(),
            bitmap: Some(vec![7u8; THUMBNAIL_SIZE * THUMBNAIL_SIZE * 4]),
            allocated: true,
            width: THUMBNAIL_SIZE as u32,
            height: THUMBNAIL_SIZE as u32,
            original_texture_width: 128,
            original_texture_height: 128,
            original_texture_mip_level_count: 1,
            original_texture_format: 2,
            date_time: 12345,
        };

        let path = std::env::temp_dir().join(format!(
            "ww3d_thumbnail_{}_{}.thu",
            std::process::id(),
            thumbnail.date_time
        ));

        assert!(manager.save_thumbnail(&thumbnail, path.to_string_lossy().as_ref()));
        let loaded = manager
            .load_thumbnail(path.to_string_lossy().as_ref())
            .expect("thumbnail should load");
        assert_eq!(loaded.width(), THUMBNAIL_SIZE as u32);
        assert_eq!(loaded.height(), THUMBNAIL_SIZE as u32);
        assert_eq!(loaded.original_texture_width(), 128);
        assert_eq!(loaded.original_texture_height(), 128);
        assert_eq!(
            loaded.bitmap().expect("bitmap"),
            vec![7u8; THUMBNAIL_SIZE * THUMBNAIL_SIZE * 4]
        );

        let _ = fs::remove_file(path);
    }

    #[test]
    fn thumbnail_compression_round_trip_preserves_pixels() {
        let mut source = vec![0u8; THUMBNAIL_SIZE * THUMBNAIL_SIZE * 4];
        for (index, value) in source.iter_mut().enumerate() {
            *value = (index % 11) as u8;
        }

        let compressed = compress_thumbnail_data(&source);
        let restored = decompress_thumbnail_data(&compressed);
        assert_eq!(restored, source);
    }
}
