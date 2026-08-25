//! ImagePacker tool chrome model (no GPUI).
//!
//! Toolbar actions bind to the shipped lib pack / `generateINIFile` path:
//! [`crate::pack_named_images_to_pages`] and [`crate::generate_mapped_image_ini`].

use crate::{PackedAtlasPage, generate_mapped_image_ini_from_pages, pack_named_images_to_pages};
use anyhow::{Context, Result};
use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

pub const DEFAULT_TARGET_SIZE: u32 = 512;
const MAX_OUTPUT_FILE_LEN: usize = 128;
const ILLEGAL_OUTPUT_CHARS: &str = "/\\:*?<>|";
const MAX_LOGS: usize = 80;

/// One packed page as shown in the Preview pages control.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChromePreviewPage {
    pub id: u32,
    pub width: u32,
    pub height: u32,
    pub sprite_count: usize,
    pub sprite_names: Vec<String>,
}

/// Toolbar / status state for the ImagePacker window.
///
/// All pack/save work goes through existing `texture_page` functions — not stubs.
#[derive(Debug, Clone)]
pub struct ImagePackerChrome {
    pub input_dirs: Vec<PathBuf>,
    pub selected_dir: Option<usize>,
    pub use_sub_folders: bool,
    pub output_file: String,
    pub target_size: u32,
    pub gutter_size: u32,
    pub gap_extend_rgb: bool,
    pub gap_gutter: bool,
    pub create_ini: bool,
    pub image_count: usize,
    pub page_count: usize,
    pub last_error: Option<String>,
    pub output_path: Option<PathBuf>,
    pub status: String,
    pub preview_page_index: usize,
    pub preview_pages: Vec<ChromePreviewPage>,
    packed_pages: Vec<PackedAtlasPage>,
    logs: Vec<String>,
}

impl Default for ImagePackerChrome {
    fn default() -> Self {
        Self::new()
    }
}

impl ImagePackerChrome {
    pub fn new() -> Self {
        Self {
            input_dirs: Vec::new(),
            selected_dir: None,
            use_sub_folders: true,
            output_file: "NewImage".to_string(),
            target_size: DEFAULT_TARGET_SIZE,
            gutter_size: 1,
            gap_extend_rgb: true,
            gap_gutter: false,
            create_ini: true,
            image_count: 0,
            page_count: 0,
            last_error: None,
            output_path: None,
            status: "Select options and click Pack.".to_string(),
            preview_page_index: 0,
            preview_pages: Vec::new(),
            packed_pages: Vec::new(),
            logs: vec!["ImagePacker chrome initialized".to_string()],
        }
    }

    pub fn logs(&self) -> &[String] {
        &self.logs
    }

    pub fn packed_pages(&self) -> &[PackedAtlasPage] {
        &self.packed_pages
    }

    pub fn current_preview_page(&self) -> Option<&ChromePreviewPage> {
        self.preview_pages.get(self.preview_page_index)
    }

    pub fn preview_label(&self) -> String {
        if self.preview_pages.is_empty() {
            "Preview pages: none".to_string()
        } else {
            format!(
                "Preview page {}/{}",
                self.preview_page_index + 1,
                self.preview_pages.len()
            )
        }
    }

    fn push_log(&mut self, message: impl Into<String>) {
        self.logs.push(message.into());
        if self.logs.len() > MAX_LOGS {
            let overflow = self.logs.len() - MAX_LOGS;
            self.logs.drain(0..overflow);
        }
    }

    fn set_error(&mut self, err: impl Into<String>) {
        let message = err.into();
        self.last_error = Some(message.clone());
        self.status = format!("Error: {message}");
        self.push_log(self.status.clone());
    }

    fn clear_error(&mut self) {
        self.last_error = None;
    }

    /// Toolbar: Add Directory.
    pub fn add_directory(&mut self, path: PathBuf) -> bool {
        if self.input_dirs.iter().any(|existing| existing == &path) {
            return false;
        }
        self.push_log(format!("Added folder: {}", path.display()));
        if self.output_file == "NewImage" {
            if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                self.output_file = sanitize_output_name(name);
            }
        }
        self.selected_dir = Some(self.input_dirs.len());
        self.input_dirs.push(path);
        true
    }

    pub fn remove_selected_directory(&mut self) -> Option<PathBuf> {
        let index = self.selected_dir?;
        if index >= self.input_dirs.len() {
            return None;
        }
        let removed = self.input_dirs.remove(index);
        self.push_log(format!("Removed folder: {}", removed.display()));
        self.selected_dir = None;
        Some(removed)
    }

    /// Toolbar: Target size (128 / 256 / 512 / custom power-of-two).
    pub fn set_target_size(&mut self, size: u32) -> Result<()> {
        if size == 0 || !size.is_power_of_two() {
            let msg = format!("target size {size} must be a power of two");
            self.set_error(&msg);
            anyhow::bail!("{msg}");
        }
        self.target_size = size;
        self.clear_error();
        self.push_log(format!("Target size set to {size}x{size}"));
        Ok(())
    }

    /// Toolbar: gutter / padding (C++ `GAP_METHOD_GUTTER` size).
    pub fn set_gutter(&mut self, gutter: u32) {
        self.gutter_size = gutter.min(64);
        self.gap_gutter = self.gutter_size > 0;
        self.push_log(format!("Gutter/padding set to {}", self.gutter_size));
    }

    pub fn nudge_gutter(&mut self, delta: i32) {
        let next = (self.gutter_size as i32 + delta).clamp(0, 64) as u32;
        self.set_gutter(next);
    }

    /// Toolbar: Preview pages — select page index (0-based).
    pub fn set_preview_page(&mut self, index: usize) {
        if self.preview_pages.is_empty() {
            self.preview_page_index = 0;
            return;
        }
        self.preview_page_index = index.min(self.preview_pages.len() - 1);
    }

    pub fn preview_next(&mut self) {
        if !self.preview_pages.is_empty() {
            self.preview_page_index = (self.preview_page_index + 1) % self.preview_pages.len();
        }
    }

    pub fn preview_prev(&mut self) {
        if !self.preview_pages.is_empty() {
            if self.preview_page_index == 0 {
                self.preview_page_index = self.preview_pages.len() - 1;
            } else {
                self.preview_page_index -= 1;
            }
        }
    }

    pub fn set_output_path(&mut self, path: PathBuf) {
        self.output_path = Some(path);
    }

    /// Pack already-loaded named top-left RGBA images via
    /// [`pack_named_images_to_pages`] (C++ `TexturePage` + 90° CW TGA blit).
    pub fn pack_named_rgba(&mut self, images: &[(String, u32, u32, Vec<u8>)]) -> Result<()> {
        self.clear_error();
        self.image_count = images.len();
        self.page_count = 0;
        self.preview_pages.clear();
        self.preview_page_index = 0;
        self.packed_pages.clear();

        if images.is_empty() {
            self.status = "No valid images to pack.".to_string();
            self.push_log(self.status.clone());
            return Ok(());
        }

        self.status = format!("Packing {} images...", images.len());
        let packed = pack_named_images_to_pages(images, self.target_size, self.gap_extend_rgb);
        if packed.is_empty() {
            let msg = format!(
                "unable to fit '{}' ({}x{}) in a TexturePage",
                images[0].0, images[0].1, images[0].2
            );
            self.set_error(&msg);
            anyhow::bail!("{msg}");
        }

        self.apply_packed_pages(packed);
        Ok(())
    }

    /// Toolbar: Pack — scan TGA folders then [`pack_named_rgba`].
    pub fn pack(&mut self) -> Result<()> {
        self.clear_error();
        if self.input_dirs.is_empty() {
            let msg = "at least one input folder is required";
            self.set_error(msg);
            anyhow::bail!("{msg}");
        }
        if self.output_file.is_empty() {
            let msg = "output filename cannot be empty";
            self.set_error(msg);
            anyhow::bail!("{msg}");
        }
        if self.output_file.len() > MAX_OUTPUT_FILE_LEN {
            let msg = format!("output filename exceeds {MAX_OUTPUT_FILE_LEN} characters");
            self.set_error(&msg);
            anyhow::bail!("{msg}");
        }
        if self
            .output_file
            .chars()
            .any(|ch| ILLEGAL_OUTPUT_CHARS.contains(ch))
        {
            let msg = format!(
                "output filename '{}' contains illegal characters: {}",
                self.output_file, ILLEGAL_OUTPUT_CHARS
            );
            self.set_error(&msg);
            anyhow::bail!("{msg}");
        }
        if self.target_size == 0 || !self.target_size.is_power_of_two() {
            let msg = format!("target size {} must be a power of two", self.target_size);
            self.set_error(&msg);
            anyhow::bail!("{msg}");
        }

        let paths = self.collect_tga_paths();
        if paths.is_empty() {
            self.image_count = 0;
            self.page_count = 0;
            self.preview_pages.clear();
            self.packed_pages.clear();
            self.status = "No images found in selected folders.".to_string();
            self.push_log(self.status.clone());
            return Ok(());
        }

        let images = self.load_tga_images(&paths)?;
        self.pack_named_rgba(&images)
    }

    /// Toolbar: Save INI — C++ `ImagePacker::generateINIFile`.
    pub fn save_ini(&mut self, path: &Path) -> Result<String> {
        self.clear_error();
        if self.packed_pages.is_empty() {
            let msg = "nothing to save; Pack first";
            self.set_error(msg);
            anyhow::bail!("{msg}");
        }
        let text = generate_mapped_image_ini_from_pages(&self.output_file, &self.packed_pages);
        if let Some(parent) = path.parent() {
            if !parent.as_os_str().is_empty() {
                fs::create_dir_all(parent)
                    .with_context(|| format!("failed creating '{}'", parent.display()))?;
            }
        }
        fs::write(path, &text).with_context(|| format!("failed writing '{}'", path.display()))?;
        self.output_path = Some(path.to_path_buf());
        self.status = format!("Wrote {}", path.display());
        self.push_log(self.status.clone());
        Ok(text)
    }

    /// INI text for the last pack (no I/O) — same generateINIFile helper.
    pub fn ini_text(&self) -> Option<String> {
        if self.packed_pages.is_empty() {
            None
        } else {
            Some(generate_mapped_image_ini_from_pages(
                &self.output_file,
                &self.packed_pages,
            ))
        }
    }

    fn apply_packed_pages(&mut self, packed: Vec<PackedAtlasPage>) {
        self.page_count = packed.len();
        self.image_count = packed.iter().map(|p| p.sprites.len()).sum();
        self.preview_pages = packed
            .iter()
            .map(|page| ChromePreviewPage {
                id: page.id,
                width: page.width,
                height: page.height,
                sprite_count: page.sprites.len(),
                sprite_names: page.sprites.iter().map(|s| s.key.clone()).collect(),
            })
            .collect();
        self.preview_page_index = 0;
        self.packed_pages = packed;
        self.status = format!(
            "Packed {} image(s) into {} page(s)",
            self.image_count, self.page_count
        );
        self.push_log(self.status.clone());
    }

    fn collect_tga_paths(&mut self) -> Vec<PathBuf> {
        let mut collected = Vec::new();
        let mut unique = HashSet::new();
        let input_dirs = self.input_dirs.clone();
        for dir in &input_dirs {
            if !dir.exists() {
                self.push_log(format!("Skipping missing directory: {}", dir.display()));
                continue;
            }
            let walker = if self.use_sub_folders {
                walkdir::WalkDir::new(dir)
            } else {
                walkdir::WalkDir::new(dir).max_depth(1)
            };
            for entry in walker.into_iter().filter_map(|e| e.ok()) {
                let path = entry.path();
                if !entry.file_type().is_file() {
                    continue;
                }
                let extension = path
                    .extension()
                    .and_then(|e| e.to_str())
                    .map(|e| e.to_ascii_lowercase())
                    .unwrap_or_default();
                if extension != "tga" {
                    continue;
                }
                let normalized = path.to_path_buf();
                if unique.insert(normalized.clone()) {
                    collected.push(normalized);
                }
            }
        }
        self.push_log(format!("Found {} candidate image file(s)", collected.len()));
        collected
    }

    fn load_tga_images(&mut self, paths: &[PathBuf]) -> Result<Vec<(String, u32, u32, Vec<u8>)>> {
        let mut images = Vec::new();
        let mut name_counts: std::collections::HashMap<String, usize> =
            std::collections::HashMap::new();
        for path in paths {
            let image = match image::open(path) {
                Ok(img) => img,
                Err(err) => {
                    self.push_log(format!("Unable to read image '{}': {err}", path.display()));
                    continue;
                }
            };
            let (width, height) = (image.width(), image.height());
            if width > self.target_size || height > self.target_size {
                self.push_log(format!(
                    "Skipping '{}' ({}x{}) larger than target {}x{}",
                    path.display(),
                    width,
                    height,
                    self.target_size,
                    self.target_size
                ));
                continue;
            }
            let rgba = image.to_rgba8();
            let base_name = path
                .file_stem()
                .and_then(|stem| stem.to_str())
                .map(sanitize_sprite_name)
                .filter(|name| !name.is_empty())
                .unwrap_or_else(|| "sprite".to_string());
            let counter = name_counts.entry(base_name.clone()).or_insert(0);
            let sprite_key = if *counter == 0 {
                base_name.clone()
            } else {
                format!("{base_name}_{counter}")
            };
            *counter += 1;
            images.push((sprite_key, width, height, rgba.into_raw()));
        }
        images.sort_by(|a, b| {
            let area_a = a.1.saturating_mul(a.2);
            let area_b = b.1.saturating_mul(b.2);
            area_b.cmp(&area_a)
        });
        Ok(images)
    }
}

fn sanitize_output_name(input: &str) -> String {
    let mut name = input
        .chars()
        .filter(|ch| !ILLEGAL_OUTPUT_CHARS.contains(*ch) && !ch.is_control())
        .collect::<String>();
    if name.is_empty() {
        name = "NewImage".to_string();
    }
    if name.len() > MAX_OUTPUT_FILE_LEN {
        name.truncate(MAX_OUTPUT_FILE_LEN);
    }
    name
}

fn sanitize_sprite_name(input: &str) -> String {
    let mut out = String::new();
    for ch in input.chars() {
        if ch.is_ascii_alphanumeric() || ch == '_' {
            out.push(ch);
        } else if !out.ends_with('_') {
            out.push('_');
        }
    }
    out.trim_matches('_').to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{PageStatus, generate_mapped_image_ini};

    fn solid_rgba(w: u32, h: u32, r: u8, g: u8, b: u8, a: u8) -> Vec<u8> {
        let mut px = Vec::with_capacity((w * h * 4) as usize);
        for _ in 0..(w * h) {
            px.extend_from_slice(&[r, g, b, a]);
        }
        px
    }

    #[test]
    fn chrome_pack_tiny_in_memory_rgba_updates_status() {
        let mut chrome = ImagePackerChrome::new();
        chrome.output_file = "ArtPack".to_string();
        chrome.set_target_size(16).unwrap();
        chrome.set_gutter(1);
        assert_eq!(chrome.gutter_size, 1);
        assert!(chrome.gap_gutter);

        let red = solid_rgba(2, 2, 255, 0, 0, 255);
        let blue = solid_rgba(2, 2, 0, 0, 255, 255);
        chrome
            .pack_named_rgba(&[
                ("ButtonUp".into(), 2, 2, red),
                ("ButtonDown".into(), 2, 2, blue),
            ])
            .expect("pack");

        assert_eq!(chrome.image_count, 2);
        assert_eq!(chrome.page_count, 1);
        assert!(chrome.last_error.is_none());
        assert_eq!(chrome.preview_pages.len(), 1);
        assert_eq!(chrome.current_preview_page().unwrap().id, 1);
        assert!(chrome.status.contains("Packed 2 image(s) into 1 page(s)"));

        chrome.preview_next();
        assert_eq!(chrome.preview_page_index, 0, "single page wraps to self");

        let ini = chrome.ini_text().expect("ini after pack");
        assert!(ini.contains("MappedImage ButtonUp"));
        assert!(ini.contains("MappedImage ButtonDown"));
        assert!(ini.contains("Texture = ArtPack_001.tga"));
        assert!(ini.contains("TextureWidth = 16"));
        assert!(ini.contains("Status = NONE"));
        // Same generateINIFile helper as texture_page tests (skip PAGE_ERROR).
        let skip = generate_mapped_image_ini(
            "ArtPack",
            &[crate::MappedImageIniPage {
                id: 99,
                width: 16,
                height: 16,
                status: PageStatus::PAGE_ERROR,
                images: vec![],
            }],
        );
        assert!(!skip.contains("MappedImage"), "PAGE_ERROR pages skipped");
    }

    #[test]
    fn chrome_save_ini_writes_generate_ini_file_and_sets_output_path() {
        let mut chrome = ImagePackerChrome::new();
        chrome.output_file = "ArtPack".to_string();
        chrome.set_target_size(8).unwrap();
        let px = solid_rgba(2, 2, 8, 16, 24, 255);
        chrome
            .pack_named_rgba(&[("Tiny".into(), 2, 2, px)])
            .unwrap();

        let dir = std::env::temp_dir().join(format!(
            "image_packer_chrome_ini_{}_{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&dir).unwrap();
        let ini_path = dir.join("ArtPack.INI");
        let text = chrome.save_ini(&ini_path).expect("save ini");
        assert!(ini_path.exists());
        let on_disk = fs::read_to_string(&ini_path).unwrap();
        assert_eq!(text, on_disk);
        assert!(on_disk.contains("MappedImage Tiny"));
        assert_eq!(chrome.output_path.as_deref(), Some(ini_path.as_path()));
        assert!(chrome.last_error.is_none());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn chrome_pack_tiny_tga_from_directory_updates_status() {
        let dir = std::env::temp_dir().join(format!(
            "image_packer_chrome_tga_{}_{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&dir).unwrap();
        let img = image::RgbaImage::from_pixel(2, 2, image::Rgba([10, 20, 30, 255]));
        let tga_path = dir.join("TinyIcon.tga");
        img.save_with_format(&tga_path, image::ImageFormat::Tga)
            .expect("write tiny TGA");

        let mut chrome = ImagePackerChrome::new();
        assert!(chrome.add_directory(dir.clone()));
        chrome.set_target_size(16).unwrap();
        chrome.pack().expect("pack tga dir");

        assert_eq!(chrome.image_count, 1);
        assert_eq!(chrome.page_count, 1);
        assert!(chrome.last_error.is_none());
        assert_eq!(
            chrome.current_preview_page().unwrap().sprite_names,
            vec!["TinyIcon".to_string()]
        );
        assert!(chrome.ini_text().unwrap().contains("MappedImage TinyIcon"));
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn chrome_pack_without_directory_sets_last_error() {
        let mut chrome = ImagePackerChrome::new();
        let err = chrome.pack().unwrap_err();
        assert!(err.to_string().contains("input folder"));
        assert!(chrome.last_error.is_some());
        assert_eq!(chrome.image_count, 0);
        assert_eq!(chrome.page_count, 0);
    }

    #[test]
    fn chrome_save_ini_before_pack_sets_last_error() {
        let mut chrome = ImagePackerChrome::new();
        let err = chrome.save_ini(Path::new("unused.INI")).unwrap_err();
        assert!(err.to_string().contains("Pack first"));
        assert!(chrome.last_error.unwrap().contains("Pack first"));
    }
}
