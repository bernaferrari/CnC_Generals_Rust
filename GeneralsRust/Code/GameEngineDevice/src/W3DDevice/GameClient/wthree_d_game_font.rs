//! W3DGameFont definitions (W3DDevice/GameClient/W3DGameFont.h)
//!
//! Provides the W3D-backed font library used by the UI layer.

use std::path::{Path, PathBuf};
use std::ptr::NonNull;
use std::sync::{Arc, Mutex, OnceLock};

use ww3d_assets::AssetManager;
use ww3d_render_2d::font_system::FontSystem;
use ww3d_renderer_3d::rendering::render2d::font3d::Font3DData;

/// Font handle for legacy-style iteration.
pub type GameFontHandle = NonNull<GameFont>;

/// Legacy-style font representation.
#[derive(Debug)]
pub struct GameFont {
    pub next: Option<GameFontHandle>,
    pub name_string: String,
    pub point_size: i32,
    pub height: i32,
    pub font_data: Option<Arc<Font3DData>>,
    pub unicode_font_data: Option<Arc<Font3DData>>,
    pub bold: bool,
    pub(crate) font_key: Option<String>,
    pub(crate) unicode_font_key: Option<String>,
}

impl GameFont {
    pub fn new(name: &str, point_size: i32, bold: bool) -> Self {
        Self {
            next: None,
            name_string: name.to_string(),
            point_size,
            height: 0,
            font_data: None,
            unicode_font_data: None,
            bold,
            font_key: None,
            unicode_font_key: None,
        }
    }
}

/// W3D font library (device-specific implementation of GameFont/FontLibrary).
#[derive(Debug)]
pub struct W3DFontLibrary {
    font_list: Option<GameFontHandle>,
    fonts: Vec<Box<GameFont>>,
    count: i32,
    pub(crate) font_system: FontSystem,
    pub(crate) search_paths: Vec<PathBuf>,
    pub(crate) unicode_font_name: String,
}

impl Default for W3DFontLibrary {
    fn default() -> Self {
        Self::new()
    }
}

impl W3DFontLibrary {
    pub fn new() -> Self {
        Self {
            font_list: None,
            fonts: Vec::new(),
            count: 0,
            font_system: FontSystem::new(),
            search_paths: default_font_search_paths(),
            unicode_font_name: "Arial Unicode MS".to_string(),
        }
    }

    pub fn init(&mut self) {
        self.reset();
    }

    pub fn reset(&mut self) {
        self.delete_all_fonts();
        self.font_system.clear();
    }

    pub fn update(&mut self) {}

    pub fn get_font(&mut self, name: &str, point_size: i32, bold: bool) -> Option<GameFontHandle> {
        if name.is_empty() {
            return None;
        }

        if let Some(existing) = self.fonts.iter_mut().find(|font| {
            font.point_size == point_size
                && font.bold == bold
                && font.name_string.eq_ignore_ascii_case(name)
        }) {
            return Some(NonNull::from(existing.as_mut()));
        }

        let mut font = Box::new(GameFont::new(name, point_size, bold));
        if !self.load_font_data(&mut font) {
            return None;
        }

        let handle = NonNull::from(font.as_mut());
        self.link_font(handle);
        self.fonts.push(font);
        self.count += 1;

        Some(handle)
    }

    pub fn first_font(&self) -> Option<GameFontHandle> {
        self.font_list
    }

    pub fn next_font(&self, font: GameFontHandle) -> Option<GameFontHandle> {
        unsafe { font.as_ref().next }
    }

    pub fn get_count(&self) -> i32 {
        self.count
    }

    pub fn set_asset_manager(&mut self, manager: Arc<AssetManager>) {
        self.font_system.set_asset_manager(manager);
    }

    pub fn set_unicode_font_name(&mut self, name: &str) {
        self.unicode_font_name = name.to_string();
    }

    pub fn add_search_path<P: Into<PathBuf>>(&mut self, path: P) {
        self.search_paths.push(path.into());
    }

    pub fn set_search_paths(&mut self, paths: Vec<PathBuf>) {
        self.search_paths = paths;
    }

    fn delete_all_fonts(&mut self) {
        for font in self.fonts.iter_mut() {
            self.release_font_data(font);
        }
        self.fonts.clear();
        self.font_list = None;
        self.count = 0;
    }

    fn link_font(&mut self, handle: GameFontHandle) {
        unsafe {
            handle.as_mut().next = self.font_list;
        }
        self.font_list = Some(handle);
    }

    pub(crate) fn resolve_font_path(&self, name: &str, bold: bool) -> Option<PathBuf> {
        let mut candidates = Vec::new();
        candidates.push(name.to_string());
        if bold {
            candidates.push(format!("{}Bold", name));
            candidates.push(format!("{} Bold", name));
            candidates.push(format!("{}-Bold", name));
        }

        let extensions = ["tga", "ttf", "otf"];

        for candidate in candidates {
            let candidate_path = Path::new(&candidate);
            if candidate_path.extension().is_some() {
                if candidate_path.exists() {
                    return Some(candidate_path.to_path_buf());
                }
            } else {
                for ext in &extensions {
                    let file_name = format!("{}.{}", candidate, ext);
                    for base in &self.search_paths {
                        let path = base.join(&file_name);
                        if path.exists() {
                            return Some(path);
                        }
                    }
                }
            }
        }

        None
    }

    pub(crate) fn build_font_key(name: &str, point_size: i32, bold: bool) -> String {
        format!(
            "{}_{}_{}",
            name,
            point_size,
            if bold { "bold" } else { "normal" }
        )
    }

    fn clone_font_key(&self, name: &str, point_size: i32, bold: bool) -> String {
        Self::build_font_key(name, point_size, bold)
    }
}

/// Global font library handle (C++: TheFontLibrary).
pub fn get_font_library() -> &'static Mutex<W3DFontLibrary> {
    static FONT_LIBRARY: OnceLock<Mutex<W3DFontLibrary>> = OnceLock::new();
    FONT_LIBRARY.get_or_init(|| Mutex::new(W3DFontLibrary::new()))
}

fn default_font_search_paths() -> Vec<PathBuf> {
    vec![
        PathBuf::from("Data/Fonts"),
        PathBuf::from("Data/Art/Fonts"),
        PathBuf::from("Data/Art"),
        PathBuf::from("Data"),
        PathBuf::from("data/Fonts"),
        PathBuf::from("data/Art/Fonts"),
        PathBuf::from("data/Art"),
        PathBuf::from("data"),
        PathBuf::from("."),
    ]
}
