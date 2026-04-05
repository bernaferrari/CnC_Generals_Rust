//! W3DGameFont.cpp implementation (font loading for W3DDevice).

use crate::w3d_device::game_client::wthree_d_game_font::{GameFont, W3DFontLibrary};
use ww3d_render_2d::font_system::FontError;

impl W3DFontLibrary {
    /// Load the font data pointer based on everything else already set.
    pub(crate) fn load_font_data(&mut self, font: &mut GameFont) -> bool {
        if font.point_size > 100 {
            return false;
        }

        let font_key =
            W3DFontLibrary::build_font_key(font.name_string.as_str(), font.point_size, font.bold);

        let font_data = if let Some(existing) = self.font_system.get_font(&font_key) {
            existing
        } else {
            let Some(path) = self.resolve_font_path(font.name_string.as_str(), font.bold) else {
                return false;
            };

            let path_str = path.to_string_lossy();
            let extension = path
                .extension()
                .and_then(|ext| ext.to_str())
                .unwrap_or("")
                .to_ascii_lowercase();

            let loaded = match extension.as_str() {
                "ttf" | "otf" => self.font_system.load_ttf_font_with_size(
                    &font_key,
                    path_str.as_ref(),
                    font.point_size as f32,
                ),
                "tga" => self.font_system.load_tga_font(&font_key, path_str.as_ref()),
                _ => self.font_system.load_font(&font_key, path_str.as_ref()),
            };

            match loaded {
                Ok(font) => font,
                Err(FontError::AlreadyLoaded(_)) => match self.font_system.get_font(&font_key) {
                    Some(font) => font,
                    None => return false,
                },
                Err(_) => return false,
            }
        };

        font.height = font_data.char_height as i32;
        font.font_data = Some(font_data);
        font.font_key = Some(font_key);

        if !self.unicode_font_name.is_empty() {
            let unicode_key = W3DFontLibrary::build_font_key(
                self.unicode_font_name.as_str(),
                font.point_size,
                font.bold,
            );

            let unicode_data = if let Some(existing) = self.font_system.get_font(&unicode_key) {
                Some(existing)
            } else {
                self.resolve_font_path(self.unicode_font_name.as_str(), font.bold)
                    .and_then(|path| {
                        let path_str = path.to_string_lossy();
                        let extension = path
                            .extension()
                            .and_then(|ext| ext.to_str())
                            .unwrap_or("")
                            .to_ascii_lowercase();

                        let loaded = match extension.as_str() {
                            "ttf" | "otf" => self.font_system.load_ttf_font_with_size(
                                &unicode_key,
                                path_str.as_ref(),
                                font.point_size as f32,
                            ),
                            "tga" => self
                                .font_system
                                .load_tga_font(&unicode_key, path_str.as_ref()),
                            _ => self.font_system.load_font(&unicode_key, path_str.as_ref()),
                        };

                        match loaded {
                            Ok(font) => Some(font),
                            Err(FontError::AlreadyLoaded(_)) => {
                                self.font_system.get_font(&unicode_key)
                            }
                            Err(_) => None,
                        }
                    })
            };

            if let Some(unicode_data) = unicode_data {
                font.unicode_font_data = Some(unicode_data);
                font.unicode_font_key = Some(unicode_key);
            }
        }

        true
    }

    /// Release the font data pointer (mirrors asset manager ref-count semantics).
    pub(crate) fn release_font_data(&mut self, font: &mut GameFont) {
        font.font_data = None;
        font.unicode_font_data = None;
        font.font_key = None;
        font.unicode_font_key = None;
    }
}
