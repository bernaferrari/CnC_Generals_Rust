//! Global language data defaults.

use std::sync::{OnceLock, RwLock};

use crate::gui::font::FontDesc;

#[derive(Debug, Clone)]
pub struct GlobalLanguageData {
    pub credits_title_font: FontDesc,
    pub credits_position_font: FontDesc,
    pub credits_normal_font: FontDesc,
}

impl Default for GlobalLanguageData {
    fn default() -> Self {
        Self {
            credits_title_font: FontDesc::new("Arial", 18, false),
            credits_position_font: FontDesc::new("Arial", 14, false),
            credits_normal_font: FontDesc::new("Arial", 12, false),
        }
    }
}

impl GlobalLanguageData {
    pub fn adjust_font_size(&self, size: i32) -> i32 {
        size
    }
}

static GLOBAL_LANGUAGE_DATA: OnceLock<RwLock<GlobalLanguageData>> = OnceLock::new();

pub fn get_global_language_data() -> &'static RwLock<GlobalLanguageData> {
    GLOBAL_LANGUAGE_DATA.get_or_init(|| RwLock::new(GlobalLanguageData::default()))
}
