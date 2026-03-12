//! INI parsing for MappedImage definitions
//!
//! This module handles parsing MappedImage entries from INI files.
//! MappedImage defines texture images that can be referenced by name in the game.
//!
//! Author: Colin Day, December 2001
//! Rust port: 2025

use crate::common::ascii_string::AsciiString;
use crate::common::ini::ini::{FieldParse, INIError, INILoadType, INIResult, INI};
use crate::common::ini::ini_game_data::get_global_data;
use once_cell::sync::OnceCell;
use parking_lot::RwLock;
use std::collections::{BTreeSet, HashMap, HashSet};
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;

/// Image status bits (keep in sync with C++ imageStatusNames)
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ImageStatus {
    pub bits: u32,
}

impl ImageStatus {
    /// No special status
    pub const NONE: u32 = 0x00000000;
    /// Image should be treated as rotated 90 degrees clockwise
    pub const ROTATED_90_CLOCKWISE: u32 = 0x00000001;
    /// Image struct contains raw texture data
    pub const RAW_TEXTURE: u32 = 0x00000002;

    pub fn new() -> Self {
        Self { bits: Self::NONE }
    }

    pub fn has_flag(&self, flag: u32) -> bool {
        (self.bits & flag) != 0
    }

    pub fn set_flag(&mut self, flag: u32) {
        self.bits |= flag;
    }

    pub fn clear_flag(&mut self, flag: u32) {
        self.bits &= !flag;
    }

    pub fn is_rotated_90_clockwise(&self) -> bool {
        self.has_flag(Self::ROTATED_90_CLOCKWISE)
    }

    pub fn has_raw_texture(&self) -> bool {
        self.has_flag(Self::RAW_TEXTURE)
    }
}

impl Default for ImageStatus {
    fn default() -> Self {
        Self::new()
    }
}

/// 2D integer coordinate
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ICoord2D {
    pub x: i32,
    pub y: i32,
}

impl ICoord2D {
    pub const ZERO: Self = Self { x: 0, y: 0 };

    pub fn new(x: i32, y: i32) -> Self {
        Self { x, y }
    }

    pub fn zero() -> Self {
        Self::ZERO
    }
}

impl Default for ICoord2D {
    fn default() -> Self {
        Self::zero()
    }
}

/// 2D region (UV coordinates)
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Region2D {
    pub left: f32,
    pub top: f32,
    pub right: f32,
    pub bottom: f32,
}

impl Region2D {
    pub fn new(left: f32, top: f32, right: f32, bottom: f32) -> Self {
        Self {
            left,
            top,
            right,
            bottom,
        }
    }

    pub fn zero() -> Self {
        Self {
            left: 0.0,
            top: 0.0,
            right: 0.0,
            bottom: 0.0,
        }
    }

    pub fn get_width(&self) -> f32 {
        self.right - self.left
    }

    pub fn get_height(&self) -> f32 {
        self.bottom - self.top
    }

    pub fn is_valid(&self) -> bool {
        self.right > self.left && self.bottom > self.top
    }
}

impl Default for Region2D {
    fn default() -> Self {
        Self::zero()
    }
}

impl From<(f32, f32, f32, f32)> for Region2D {
    fn from(tuple: (f32, f32, f32, f32)) -> Self {
        Self::new(tuple.0, tuple.1, tuple.2, tuple.3)
    }
}

/// Image bitmap information
///
/// High level representation of images for referencing textures by name in the game.
/// This provides a way to refer to images in various systems including the GUI.
#[derive(Debug, Clone)]
pub struct Image {
    /// Name for this image
    pub name: String,
    /// Texture filename this image is in
    pub filename: String,
    /// Size of the texture this image is a part of
    pub texture_size: ICoord2D,
    /// Texture UV coords for image
    pub uv_coords: Region2D,
    /// Dimensions of image
    pub image_size: ICoord2D,
    /// Raw texture data (if any)
    pub raw_texture_data: Option<Vec<u8>>,
    /// Status bits from ImageStatus
    pub status: ImageStatus,
}

impl Default for Image {
    fn default() -> Self {
        Self::new()
    }
}

impl Image {
    /// Create a new Image instance
    pub fn new() -> Self {
        Self {
            name: String::new(),
            filename: String::new(),
            texture_size: ICoord2D::ZERO,
            // C++ parity: default UV spans the full texture.
            uv_coords: Region2D::new(0.0, 0.0, 1.0, 1.0),
            image_size: ICoord2D::ZERO,
            raw_texture_data: None,
            status: ImageStatus::new(),
        }
    }

    /// Set image name
    pub fn set_name(&mut self, name: String) {
        self.name = name;
    }

    /// Get image name
    pub fn get_name(&self) -> &str {
        &self.name
    }

    /// Set filename
    pub fn set_filename(&mut self, filename: String) {
        self.filename = filename;
    }

    /// Get filename
    pub fn get_filename(&self) -> &str {
        &self.filename
    }

    /// Set UV coordinate range
    pub fn set_uv(&mut self, uv: Region2D) {
        self.uv_coords = uv;
    }

    /// Get UV coords
    pub fn get_uv(&self) -> Region2D {
        self.uv_coords
    }

    /// Set width of texture page this image is on
    pub fn set_texture_width(&mut self, width: i32) {
        self.texture_size.x = width;
    }

    /// Set height of texture page this image is on
    pub fn set_texture_height(&mut self, height: i32) {
        self.texture_size.y = height;
    }

    /// Get the texture size
    pub fn get_texture_size(&self) -> ICoord2D {
        self.texture_size
    }

    /// Set image width and height
    pub fn set_image_size(&mut self, size: ICoord2D) {
        self.image_size = size;
    }

    /// Get image size
    pub fn get_image_size(&self) -> ICoord2D {
        self.image_size
    }

    /// Get image width
    pub fn get_image_width(&self) -> i32 {
        self.image_size.x
    }

    /// Get image height
    pub fn get_image_height(&self) -> i32 {
        self.image_size.y
    }

    /// Set raw texture data
    pub fn set_raw_texture_data(&mut self, data: Vec<u8>) {
        self.raw_texture_data = Some(data);
        self.status.set_flag(ImageStatus::RAW_TEXTURE);
    }

    /// Get raw texture data
    pub fn get_raw_texture_data(&self) -> Option<&Vec<u8>> {
        self.raw_texture_data.as_ref()
    }

    /// Clear raw texture data
    pub fn clear_raw_texture_data(&mut self) {
        self.raw_texture_data = None;
        self.status.clear_flag(ImageStatus::RAW_TEXTURE);
    }

    /// Set status bit
    pub fn set_status(&mut self, bit: u32) {
        self.status.set_flag(bit);
    }

    /// Clear status bit
    pub fn clear_status(&mut self, bit: u32) {
        self.status.clear_flag(bit);
    }

    /// Get status bits
    pub fn get_status(&self) -> u32 {
        self.status.bits
    }

    /// Check if image has valid dimensions
    pub fn is_valid(&self) -> bool {
        !self.name.is_empty()
            && !self.filename.is_empty()
            && self.image_size.x > 0
            && self.image_size.y > 0
    }

    /// Get aspect ratio
    pub fn get_aspect_ratio(&self) -> f32 {
        if self.image_size.y == 0 {
            1.0
        } else {
            self.image_size.x as f32 / self.image_size.y as f32
        }
    }

    /// Parse from INI file.
    pub fn parse_from_ini(&mut self, ini: &mut INI) -> Result<(), String> {
        ini.init_from_ini_with_fields(self, FIELD_PARSE_TABLE)
            .map_err(|error| error.to_string())
    }
}

/// Image parsing functions

/// Parse image coordinates from INI
pub fn parse_image_coords(_ini: &mut INI, image: &mut Image, args: &[&str]) -> INIResult<()> {
    let mut left: Option<i32> = None;
    let mut top: Option<i32> = None;
    let mut right: Option<i32> = None;
    let mut bottom: Option<i32> = None;

    let mut i = 0usize;
    while i < args.len() {
        let token = args[i];
        let mut consumed_next = false;
        let mut key = "";
        let mut value_str: Option<&str> = None;

        if let Some((raw_key, raw_value)) = token.split_once(':') {
            key = raw_key;
            if raw_value.is_empty() {
                let next = args.get(i + 1).ok_or(INIError::InvalidData)?;
                value_str = Some(*next);
                consumed_next = true;
            } else {
                value_str = Some(raw_value);
            }
        } else {
            let raw_key = token.trim_end_matches(':');
            if matches!(
                raw_key.to_ascii_uppercase().as_str(),
                "LEFT" | "TOP" | "RIGHT" | "BOTTOM"
            ) {
                let next = args.get(i + 1).ok_or(INIError::InvalidData)?;
                key = raw_key;
                value_str = Some(*next);
                consumed_next = true;
            }
        }

        if let Some(value_str) = value_str {
            let value = INI::parse_int(value_str)?;
            match key.to_ascii_uppercase().as_str() {
                "LEFT" => left = Some(value),
                "TOP" => top = Some(value),
                "RIGHT" => right = Some(value),
                "BOTTOM" => bottom = Some(value),
                _ => {}
            }
        }

        i += if consumed_next { 2 } else { 1 };
    }

    let left = left.ok_or(INIError::InvalidData)? as f32;
    let top = top.ok_or(INIError::InvalidData)? as f32;
    let right = right.ok_or(INIError::InvalidData)? as f32;
    let bottom = bottom.ok_or(INIError::InvalidData)? as f32;

    let texture_size = image.get_texture_size();
    let mut uv = Region2D::new(left, top, right, bottom);
    if texture_size.x != 0 {
        uv.left /= texture_size.x as f32;
        uv.right /= texture_size.x as f32;
    }
    if texture_size.y != 0 {
        uv.top /= texture_size.y as f32;
        uv.bottom /= texture_size.y as f32;
    }
    image.set_uv(uv);

    let image_size = ICoord2D::new((right - left) as i32, (bottom - top) as i32);
    image.set_image_size(image_size);
    Ok(())
}

/// Parse image status flags from INI
pub fn parse_image_status(_ini: &mut INI, image: &mut Image, args: &[&str]) -> INIResult<()> {
    if args.is_empty() {
        return Err(INIError::InvalidData);
    }

    // C++ parity: parseBitString32 against imageStatusNames[].
    let flat_tokens: Vec<&str> = args
        .iter()
        .flat_map(|arg| arg.split(&[',', '|'][..]).filter(|part| !part.is_empty()))
        .collect();
    let image_status_names = ["ROTATED_90_CLOCKWISE", "RAW_TEXTURE"];
    image.status.bits = INI::parse_bit_string_32(&flat_tokens, &image_status_names)?;

    if image.status.has_flag(ImageStatus::ROTATED_90_CLOCKWISE) {
        let image_size = ICoord2D::new(image.get_image_height(), image.get_image_width());
        image.set_image_size(image_size);
    }

    Ok(())
}

/// Field parse table for Image.
pub const FIELD_PARSE_TABLE: &[FieldParse<Image>] = &[
    FieldParse {
        token: "Texture",
        parse: parse_texture,
    },
    FieldParse {
        token: "TextureWidth",
        parse: parse_texture_width,
    },
    FieldParse {
        token: "TextureHeight",
        parse: parse_texture_height,
    },
    FieldParse {
        token: "Coords",
        parse: parse_image_coords,
    },
    FieldParse {
        token: "Status",
        parse: parse_image_status,
    },
];

/// Parse texture filename
pub fn parse_texture(ini: &mut INI, image: &mut Image, args: &[&str]) -> INIResult<()> {
    if args.is_empty() {
        return Err(INIError::InvalidData);
    }
    let filename = INI::parse_ascii_string(&args.join(" "))?;
    image.set_filename(filename);
    Ok(())
}

/// Parse texture width
pub fn parse_texture_width(_ini: &mut INI, image: &mut Image, args: &[&str]) -> INIResult<()> {
    let value = args.first().ok_or(INIError::InvalidData)?;
    image.set_texture_width(INI::parse_int(value)?);
    Ok(())
}

/// Parse texture height
pub fn parse_texture_height(_ini: &mut INI, image: &mut Image, args: &[&str]) -> INIResult<()> {
    let value = args.first().ok_or(INIError::InvalidData)?;
    image.set_texture_height(INI::parse_int(value)?);
    Ok(())
}

/// A collection of images
///
/// Manages a collection of named images for quick lookup and management.
/// Images are stored by name key for fast access.
#[derive(Debug)]
pub struct ImageCollection {
    /// Map of images by name key
    images: HashMap<String, Image>,
}

impl Default for ImageCollection {
    fn default() -> Self {
        Self::new()
    }
}

impl ImageCollection {
    fn normalize_key(name: &str) -> String {
        name.to_ascii_lowercase()
    }

    /// Create a new ImageCollection
    pub fn new() -> Self {
        Self {
            images: HashMap::new(),
        }
    }

    /// Initialize system
    pub fn init(&mut self) {
        self.images.clear();
        log::debug!("Mapped ImageCollection initialized");
    }

    /// Reset system
    pub fn reset(&mut self) {
        self.images.clear();
        log::debug!("Mapped ImageCollection reset");
    }

    /// Update system (called per frame)
    pub fn update(&mut self) {
        // Update logic here if needed
    }

    /// Load images with specified texture size
    pub fn load(&mut self, texture_size: i32) {
        self.clear();
        load_global_mapped_image_collection(texture_size);
        if let Some(global) = get_mapped_image_collection() {
            let global = global.read();
            for index in 0..global.len() {
                if let Some(image) = global.enum_image(index) {
                    self.add_image(image.clone());
                }
            }
        }
    }

    /// Load global images with specified texture size into the singleton collection.
    pub fn load_global(texture_size: i32) {
        load_global_mapped_image_collection(texture_size);
    }

    /// Find image based on name
    ///
    /// # Arguments
    /// * `name` - Name of the image to find
    ///
    /// # Returns
    /// Reference to the image if found, None otherwise
    pub fn find_image_by_name(&self, name: &str) -> Option<&Image> {
        self.images.get(&Self::normalize_key(name))
    }

    /// Add image to the collection (transfers ownership)
    ///
    /// # Arguments
    /// * `image` - Image to add to collection
    pub fn add_image(&mut self, image: Image) {
        let key = Self::normalize_key(image.get_name());
        self.images.insert(key, image);
    }

    /// Clear all images from the collection
    pub fn clear(&mut self) {
        self.images.clear();
    }

    /// Remove image from collection
    ///
    /// # Arguments
    /// * `name` - Name of image to remove
    ///
    /// # Returns
    /// The removed image if it existed
    pub fn remove_image(&mut self, name: &str) -> Option<Image> {
        self.images.remove(&Self::normalize_key(name))
    }

    /// Get image names
    pub fn get_image_names(&self) -> Vec<&String> {
        self.images.keys().collect()
    }

    /// Get number of images
    pub fn len(&self) -> usize {
        self.images.len()
    }

    /// Check if collection is empty
    pub fn is_empty(&self) -> bool {
        self.images.is_empty()
    }

    /// Enumerate images by index (for iteration)
    pub fn enum_image(&self, index: usize) -> Option<&Image> {
        if index < self.images.len() {
            self.images.values().nth(index)
        } else {
            None
        }
    }

    /// Get all images with a specific status flag
    pub fn find_images_with_status(&self, status_flag: u32) -> Vec<&Image> {
        self.images
            .values()
            .filter(|image| image.status.has_flag(status_flag))
            .collect()
    }

    /// Get images using a specific texture file
    pub fn find_images_by_texture(&self, texture_filename: &str) -> Vec<&Image> {
        self.images
            .values()
            .filter(|image| image.get_filename() == texture_filename)
            .collect()
    }
}

fn load_global_mapped_image_collection(texture_size: i32) {
    let mut ini = INI::new();
    let collection_handle = ensure_mapped_image_collection();
    collection_handle.write().clear();
    let mapped_image_dirs = discover_mapped_image_source_dirs(texture_size);
    for dir in mapped_image_dirs {
        load_ini_directory_recursive(&mut ini, &dir);
    }
}

/// Global mapped image collection instance (thread-safe)
static MAPPED_IMAGE_COLLECTION: OnceCell<Arc<RwLock<ImageCollection>>> = OnceCell::new();

/// Ensure the mapped image collection exists and return a handle to it
pub fn ensure_mapped_image_collection() -> Arc<RwLock<ImageCollection>> {
    MAPPED_IMAGE_COLLECTION
        .get_or_init(|| {
            let mut collection = ImageCollection::new();
            collection.init();
            Arc::new(RwLock::new(collection))
        })
        .clone()
}

/// Initialize (or reinitialize) the global mapped image collection
pub fn init_global_mapped_image_collection() {
    let collection = ensure_mapped_image_collection();
    collection.write().init();
}

/// Get a handle to the mapped image collection if it has been initialized
pub fn get_mapped_image_collection() -> Option<Arc<RwLock<ImageCollection>>> {
    MAPPED_IMAGE_COLLECTION.get().cloned()
}

/// INI parsing function for MappedImage definition (matches C++ interface)
///
/// This is the main entry point for parsing MappedImage definitions from INI files
pub fn parse_mapped_image_definition(ini: &mut INI) -> Result<(), String> {
    let tokens = ini.get_line_tokens();
    let name = tokens
        .iter()
        .skip(1)
        .find(|token| **token != "=")
        .ok_or("Expected image name but found none")?
        .to_string();

    // Check if we have the mapped image collection available
    if get_mapped_image_collection().is_none() {
        // We don't need it if we're in the builder... which doesn't have this.
        return Ok(());
    }

    // Find existing image if present.
    // C++ parity: image entries are parsed in-place and existing raw texture data
    // only triggers a debug assert (non-fatal in release builds).
    let collection_handle = ensure_mapped_image_collection();
    let key = ImageCollection::normalize_key(name.as_str());
    let mut collection = collection_handle.write();
    if !collection.images.contains_key(&key) {
        let mut new_image = Image::new();
        new_image.set_name(name.clone());
        collection.images.insert(key.clone(), new_image);
    }

    let image = collection
        .images
        .get_mut(&key)
        .ok_or_else(|| format!("MappedImage '{}' missing after creation", name))?;
    if image.get_raw_texture_data().is_some() {
        log::warn!(
            "MappedImage '{}' parsed over existing raw texture data (C++ debug-assert parity)",
            name
        );
    }

    // Parse the INI definition using field table (in-place parity with C++).
    image.parse_from_ini(ini)?;

    Ok(())
}

fn directory_has_ini_files(dir: &Path) -> bool {
    let Ok(entries) = fs::read_dir(dir) else {
        return false;
    };

    entries.flatten().any(|entry| {
        entry
            .path()
            .extension()
            .and_then(|ext| ext.to_str())
            .is_some_and(|ext| ext.eq_ignore_ascii_case("ini"))
    })
}

fn push_unique_dir(dirs: &mut Vec<PathBuf>, seen: &mut HashSet<PathBuf>, path: PathBuf) {
    if !path.is_dir() {
        return;
    }
    let canonical = fs::canonicalize(&path).unwrap_or(path);
    if seen.insert(canonical.clone()) {
        dirs.push(canonical);
    }
}

fn discover_mapped_image_source_dirs(texture_size: i32) -> Vec<PathBuf> {
    let mut roots = BTreeSet::new();
    if let Ok(cwd) = env::current_dir() {
        for ancestor in cwd.ancestors() {
            roots.insert(ancestor.to_path_buf());
        }
    }
    if let Ok(exe) = env::current_exe() {
        if let Some(parent) = exe.parent() {
            for ancestor in parent.ancestors() {
                roots.insert(ancestor.to_path_buf());
            }
        }
    }

    if let Some(global_data) = get_global_data() {
        let global = global_data.read();
        let user_data_root = global.get_path_user_data().to_string();
        if !user_data_root.trim().is_empty() {
            roots.insert(PathBuf::from(user_data_root.trim()));
        }
    }
    let mod_dir = crate::common::global_data::read().writable.mod_dir.clone();
    if !mod_dir.trim().is_empty() {
        let mod_root = PathBuf::from(mod_dir.trim());
        roots.insert(mod_root.clone());
        if let Ok(canonical) = fs::canonicalize(&mod_root) {
            roots.insert(canonical);
        }
    }

    let mut dirs = Vec::new();
    let mut seen = HashSet::new();

    // C++ parity: user-created mapped images load first.
    if let Some(global_data) = get_global_data() {
        let user_data_root = global_data.read().get_path_user_data().to_string();
        if !user_data_root.trim().is_empty() {
            push_unique_dir(
                &mut dirs,
                &mut seen,
                PathBuf::from(user_data_root.trim())
                    .join("INI")
                    .join("MappedImages"),
            );
        }
    }

    for root in roots {
        let direct_ini_root = root.join("Data").join("INI");
        push_unique_dir(
            &mut dirs,
            &mut seen,
            direct_ini_root
                .join("MappedImages")
                .join(format!("TextureSize_{texture_size}")),
        );
        push_unique_dir(
            &mut dirs,
            &mut seen,
            direct_ini_root.join("MappedImages").join("HandCreated"),
        );

        for extracted_root in [
            root.join("windows_game/extracted_big_files/INIZH"),
            root.join("windows_game/extracted_big_files_v2/INIZH"),
        ] {
            let ini_root = extracted_root.join("Data").join("INI");
            push_unique_dir(
                &mut dirs,
                &mut seen,
                ini_root
                    .join("MappedImages")
                    .join(format!("TextureSize_{texture_size}")),
            );
            push_unique_dir(
                &mut dirs,
                &mut seen,
                ini_root.join("MappedImages").join("HandCreated"),
            );
        }
    }

    dirs
}

fn collect_ini_files_recursive(dir: &Path, files: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };

    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_ini_files_recursive(&path, files);
            continue;
        }

        if path
            .extension()
            .and_then(|ext| ext.to_str())
            .is_some_and(|ext| ext.eq_ignore_ascii_case("ini"))
        {
            files.push(path);
        }
    }
}

fn load_ini_directory_recursive(ini: &mut INI, dir: &Path) {
    if !dir.is_dir() {
        return;
    }

    let mut files = Vec::new();
    collect_ini_files_recursive(dir, &mut files);
    files.sort();

    for file in files {
        if let Err(error) = ini.load(&file, INILoadType::Overwrite) {
            log::warn!(
                "MappedImage: failed to load INI '{}' ({:?})",
                file.display(),
                error
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn test_image_creation() {
        let mut image = Image::new();
        assert!(image.get_name().is_empty());
        assert!(image.get_filename().is_empty());
        assert_eq!(image.get_image_width(), 0);
        assert_eq!(image.get_image_height(), 0);

        image.set_name("test_image".to_string());
        image.set_filename("test_texture.tga".to_string());
        image.set_image_size(ICoord2D::new(64, 64));

        assert_eq!(image.get_name(), "test_image");
        assert_eq!(image.get_filename(), "test_texture.tga");
        assert_eq!(image.get_image_width(), 64);
        assert_eq!(image.get_image_height(), 64);
        assert_eq!(image.get_aspect_ratio(), 1.0);
    }

    #[test]
    fn test_image_status() {
        let mut image = Image::new();
        assert_eq!(image.get_status(), ImageStatus::NONE);

        image.set_status(ImageStatus::ROTATED_90_CLOCKWISE);
        assert!(image.status.is_rotated_90_clockwise());
        assert!(!image.status.has_raw_texture());

        image.clear_status(ImageStatus::ROTATED_90_CLOCKWISE);
        assert!(!image.status.is_rotated_90_clockwise());
    }

    #[test]
    fn test_image_texture_data() {
        let mut image = Image::new();
        assert!(!image.status.has_raw_texture());
        assert!(image.get_raw_texture_data().is_none());

        let texture_data = vec![255u8; 1024]; // Simple texture data
        image.set_raw_texture_data(texture_data.clone());

        assert!(image.status.has_raw_texture());
        assert!(image.get_raw_texture_data().is_some());
        assert_eq!(image.get_raw_texture_data().unwrap().len(), 1024);

        image.clear_raw_texture_data();
        assert!(!image.status.has_raw_texture());
        assert!(image.get_raw_texture_data().is_none());
    }

    #[test]
    fn test_image_validation() {
        let mut image = Image::new();
        assert!(!image.is_valid()); // Empty name and filename

        image.set_name("test".to_string());
        assert!(!image.is_valid()); // Still empty filename

        image.set_filename("test.tga".to_string());
        assert!(!image.is_valid()); // Still zero dimensions

        image.set_image_size(ICoord2D::new(32, 32));
        assert!(image.is_valid()); // Now valid
    }

    #[test]
    fn test_coords_and_regions() {
        let coord = ICoord2D::new(100, 200);
        assert_eq!(coord.x, 100);
        assert_eq!(coord.y, 200);

        let region = Region2D::new(0.0, 0.0, 1.0, 1.0);
        assert_eq!(region.get_width(), 1.0);
        assert_eq!(region.get_height(), 1.0);
        assert!(region.is_valid());

        let invalid_region = Region2D::new(1.0, 1.0, 0.0, 0.0);
        assert!(!invalid_region.is_valid());
    }

    #[test]
    fn test_image_collection() {
        let mut collection = ImageCollection::new();
        assert!(collection.is_empty());
        assert_eq!(collection.len(), 0);

        let mut image1 = Image::new();
        image1.set_name("image1".to_string());
        image1.set_filename("texture1.tga".to_string());
        collection.add_image(image1);

        let mut image2 = Image::new();
        image2.set_name("image2".to_string());
        image2.set_filename("texture2.tga".to_string());
        image2.set_status(ImageStatus::ROTATED_90_CLOCKWISE);
        collection.add_image(image2);

        assert_eq!(collection.len(), 2);
        assert!(!collection.is_empty());

        assert!(collection.find_image_by_name("image1").is_some());
        assert!(collection.find_image_by_name("nonexistent").is_none());

        let rotated_images = collection.find_images_with_status(ImageStatus::ROTATED_90_CLOCKWISE);
        assert_eq!(rotated_images.len(), 1);

        let texture1_images = collection.find_images_by_texture("texture1.tga");
        assert_eq!(texture1_images.len(), 1);

        let names = collection.get_image_names();
        assert_eq!(names.len(), 2);
    }

    #[test]
    fn test_image_collection_enumeration() {
        let mut collection = ImageCollection::new();

        let mut image1 = Image::new();
        image1.set_name("first".to_string());
        collection.add_image(image1);

        let mut image2 = Image::new();
        image2.set_name("second".to_string());
        collection.add_image(image2);

        assert!(collection.enum_image(0).is_some());
        assert!(collection.enum_image(1).is_some());
        assert!(collection.enum_image(2).is_none());
    }

    #[test]
    fn test_global_collection() {
        init_global_mapped_image_collection();

        let collection_handle = ensure_mapped_image_collection();
        {
            let mut collection = collection_handle.write();
            collection.clear();

            let mut image = Image::new();
            image.set_name("global_test".to_string());
            collection.add_image(image);
        }

        let collection = collection_handle.read();
        assert_eq!(collection.len(), 1);
        assert!(collection.find_image_by_name("global_test").is_some());
    }

    #[test]
    fn test_field_parse_table() {
        assert!(!FIELD_PARSE_TABLE.is_empty());

        // Check that expected fields are present
        let field_names: Vec<&str> = FIELD_PARSE_TABLE.iter().map(|f| f.token).collect();
        assert!(field_names.contains(&"Texture"));
        assert!(field_names.contains(&"TextureWidth"));
        assert!(field_names.contains(&"TextureHeight"));
        assert!(field_names.contains(&"Coords"));
        assert!(field_names.contains(&"Status"));
    }

    #[test]
    fn test_parse_mapped_image_definition_allows_existing_raw_texture() {
        init_global_mapped_image_collection();
        let collection_handle = ensure_mapped_image_collection();
        {
            let mut collection = collection_handle.write();
            collection.clear();
            let mut existing = Image::new();
            existing.set_name("RawTextureImage".to_string());
            existing.set_filename("old_texture.tga".to_string());
            existing.set_raw_texture_data(vec![1, 2, 3, 4]);
            collection.add_image(existing);
        }

        let mut ini = INI::new();
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let ini_path = std::env::temp_dir().join(format!("mapped_image_raw_{unique}.ini"));
        let ini_text = "\
MappedImage RawTextureImage
  Texture = new_texture.tga
  TextureWidth = 256
  TextureHeight = 128
  Coords = Left:0 Top:0 Right:64 Bottom:32
End
";
        std::fs::write(&ini_path, ini_text).expect("write mapped image ini");
        let load_result = ini.load(&ini_path, INILoadType::Overwrite);
        let _ = std::fs::remove_file(&ini_path);

        assert!(
            load_result.is_ok(),
            "mapped image parse should not fail over raw texture data"
        );

        let collection = collection_handle.read();
        let parsed = collection
            .find_image_by_name("RawTextureImage")
            .expect("parsed mapped image should exist");
        assert_eq!(parsed.get_filename(), "new_texture.tga");
        assert!(parsed.get_raw_texture_data().is_some());
        assert_eq!(parsed.get_texture_size(), ICoord2D::new(256, 128));
        assert_eq!(parsed.get_image_size(), ICoord2D::new(64, 32));
    }
}
