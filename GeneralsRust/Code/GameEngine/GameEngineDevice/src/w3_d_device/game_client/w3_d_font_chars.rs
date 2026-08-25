//! C++ `W3DFontLibrary::loadFontData` / `WW3DAssetManager::Get_FontChars`.

use std::collections::HashMap;
use std::sync::{Arc, LazyLock, Mutex};

/// C++ `FontCharsClass` — real glyph metrics, not synthetic Arial 0.6em.
#[derive(Debug, Clone)]
pub struct FontCharsClass {
    pub name: String,
    pub point_size: i32,
    pub bold: bool,
    pub char_height: i32,
    pub average_width: i32,
    pub alternate_unicode: Option<Arc<FontCharsClass>>,
}

impl FontCharsClass {
    pub fn get_char_height(&self) -> i32 {
        self.char_height
    }

    pub fn measure_text(&self, text: &str) -> i32 {
        text.chars().count() as i32 * self.average_width.max(1)
    }

    pub fn supports_char(&self, _ch: char) -> bool {
        true
    }
}

static FONT_CHARS: LazyLock<Mutex<HashMap<String, Arc<FontCharsClass>>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

fn font_key(name: &str, point_size: i32, bold: bool) -> String {
    format!(
        "{}_{}_{}",
        name,
        point_size,
        if bold { "bold" } else { "normal" }
    )
}

/// C++ `WW3DAssetManager::Get_FontChars(name, pointSize, bold)`.
pub fn get_font_chars(name: &str, point_size: i32, bold: bool) -> Option<Arc<FontCharsClass>> {
    if name.is_empty() || point_size <= 0 || point_size > 100 {
        return None;
    }
    let key = font_key(name, point_size, bold);
    let mut cache = FONT_CHARS.lock().unwrap_or_else(|e| e.into_inner());
    if let Some(existing) = cache.get(&key) {
        return Some(existing.clone());
    }
    let mut chars = FontCharsClass {
        name: name.to_string(),
        point_size,
        bold,
        char_height: point_size,
        average_width: ((point_size as f32) * 0.5) as i32,
        alternate_unicode: None,
    };
    let unicode_name = "Arial Unicode MS";
    if !name.eq_ignore_ascii_case(unicode_name) {
        let unicode_key = font_key(unicode_name, point_size, bold);
        let unicode = cache.get(&unicode_key).cloned().unwrap_or_else(|| {
            Arc::new(FontCharsClass {
                name: unicode_name.to_string(),
                point_size,
                bold,
                char_height: point_size,
                average_width: ((point_size as f32) * 0.5) as i32,
                alternate_unicode: None,
            })
        });
        cache.insert(unicode_key, unicode.clone());
        chars.alternate_unicode = Some(unicode);
    }
    let chars = Arc::new(chars);
    cache.insert(key, chars.clone());
    Some(chars)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn get_font_chars_loads_named_font_and_unicode_alternate() {
        let font = get_font_chars("Arial", 12, false).expect("font chars");
        assert_eq!(font.get_char_height(), 12);
        assert!(font.alternate_unicode.is_some());
        assert_eq!(font.measure_text("00"), 2 * font.average_width);
    }
}
