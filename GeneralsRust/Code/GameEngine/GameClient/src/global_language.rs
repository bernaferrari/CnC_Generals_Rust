//! Global language data — C++ `GlobalLanguage.cpp` parity.
//!
//! Loads `Data/<lang>/Language.ini`, copies parsed FontDesc fields, and
//! scales UI point sizes by `xResolution/800` clamped to [1, 2] using
//! `ResolutionFontAdjustment` (default 0.7).

use std::sync::{LazyLock, RwLock};

use game_engine::common::ini::ini_game_data::get_global_data;
use game_engine::common::ini::ini_language::{
    FontDesc as CommonFontDesc, get_global_language_read, init_global_language,
};
use game_engine::common::ini::ini_webpage_url::get_registry_language;
use game_engine::common::ini::{INI, INILoadType};

use crate::gui::font::FontDesc;

#[derive(Debug, Clone)]
pub struct GlobalLanguageData {
    pub unicode_font_name: String,
    pub use_hard_wrap: bool,
    pub military_caption_speed: i32,
    pub military_caption_delay_ms: i32,
    pub resolution_font_size_adjustment: f32,
    pub copyright_font: FontDesc,
    pub message_font: FontDesc,
    pub military_caption_title_font: FontDesc,
    pub military_caption_font: FontDesc,
    pub superweapon_countdown_normal_font: FontDesc,
    pub superweapon_countdown_ready_font: FontDesc,
    pub named_timer_countdown_normal_font: FontDesc,
    pub named_timer_countdown_ready_font: FontDesc,
    pub drawable_caption_font: FontDesc,
    pub default_window_font: FontDesc,
    pub default_display_string_font: FontDesc,
    pub tooltip_font: FontDesc,
    pub native_debug_display_font: FontDesc,
    pub draw_group_info_font: FontDesc,
    pub credits_title_font: FontDesc,
    pub credits_position_font: FontDesc,
    pub credits_normal_font: FontDesc,
    pub local_fonts: Vec<String>,
}

impl Default for GlobalLanguageData {
    fn default() -> Self {
        Self {
            unicode_font_name: String::new(),
            use_hard_wrap: false,
            military_caption_speed: 0,
            military_caption_delay_ms: 750,
            resolution_font_size_adjustment: 0.7,
            copyright_font: FontDesc::default(),
            message_font: FontDesc::default(),
            military_caption_title_font: FontDesc::default(),
            military_caption_font: FontDesc::default(),
            superweapon_countdown_normal_font: FontDesc::default(),
            superweapon_countdown_ready_font: FontDesc::default(),
            named_timer_countdown_normal_font: FontDesc::default(),
            named_timer_countdown_ready_font: FontDesc::default(),
            drawable_caption_font: FontDesc::default(),
            default_window_font: FontDesc::default(),
            default_display_string_font: FontDesc::default(),
            tooltip_font: FontDesc::default(),
            native_debug_display_font: FontDesc::default(),
            draw_group_info_font: FontDesc::default(),
            credits_title_font: FontDesc::new("Arial", 18, false),
            credits_position_font: FontDesc::new("Arial", 14, false),
            credits_normal_font: FontDesc::new("Arial", 12, false),
            local_fonts: Vec::new(),
        }
    }
}

impl GlobalLanguageData {
    /// C++ `GlobalLanguage::adjustFontSize`.
    pub fn adjust_font_size(&self, size: i32) -> i32 {
        let x_resolution = get_global_data()
            .map(|data| data.read().x_resolution as f32)
            .unwrap_or(800.0);
        let mut adjust_factor = x_resolution / 800.0;
        adjust_factor = 1.0 + (adjust_factor - 1.0) * self.resolution_font_size_adjustment;
        if adjust_factor < 1.0 {
            adjust_factor = 1.0;
        }
        if adjust_factor > 2.0 {
            adjust_factor = 2.0;
        }
        (size as f32 * adjust_factor).floor() as i32
    }

    /// C++ `GlobalLanguage::init` — load `Data/<lang>/Language.ini`.
    pub fn init(&mut self) {
        init_global_language();
        let language = get_registry_language();
        let lang = language.as_str().trim();
        let lang = if lang.is_empty() { "English" } else { lang };
        let path = format!("Data/{lang}/Language.ini");
        let mut ini = INI::new();
        if let Err(err) = ini.load(&path, INILoadType::Overwrite) {
            log::warn!("GlobalLanguage::init failed to load '{path}': {err}");
        }
        self.sync_from_common();
    }

    fn sync_from_common(&mut self) {
        let Some(common) = get_global_language_read() else {
            return;
        };
        self.unicode_font_name = common.unicode_font_name.clone();
        self.use_hard_wrap = common.use_hard_wrap;
        self.military_caption_speed = common.military_caption_speed;
        self.military_caption_delay_ms = common.military_caption_delay_ms;
        self.resolution_font_size_adjustment = common.resolution_font_size_adjustment;
        self.copyright_font = to_client_font(&common.copyright_font);
        self.message_font = to_client_font(&common.message_font);
        self.military_caption_title_font = to_client_font(&common.military_caption_title_font);
        self.military_caption_font = to_client_font(&common.military_caption_font);
        self.superweapon_countdown_normal_font =
            to_client_font(&common.superweapon_countdown_normal_font);
        self.superweapon_countdown_ready_font =
            to_client_font(&common.superweapon_countdown_ready_font);
        self.named_timer_countdown_normal_font =
            to_client_font(&common.named_timer_countdown_normal_font);
        self.named_timer_countdown_ready_font =
            to_client_font(&common.named_timer_countdown_ready_font);
        self.drawable_caption_font = to_client_font(&common.drawable_caption_font);
        self.default_window_font = to_client_font(&common.default_window_font);
        self.default_display_string_font = to_client_font(&common.default_display_string_font);
        self.tooltip_font = to_client_font(&common.tooltip_font);
        self.native_debug_display_font = to_client_font(&common.native_debug_display_font);
        self.draw_group_info_font = to_client_font(&common.draw_group_info_font);
        if !common.credits_title_font.name.is_empty() {
            self.credits_title_font = to_client_font(&common.credits_title_font);
        }
        if !common.credits_position_font.name.is_empty() {
            self.credits_position_font = to_client_font(&common.credits_position_font);
        }
        if !common.credits_normal_font.name.is_empty() {
            self.credits_normal_font = to_client_font(&common.credits_normal_font);
        }
        self.local_fonts = common.local_fonts.clone();
    }
}

fn to_client_font(font: &CommonFontDesc) -> FontDesc {
    FontDesc::new(&font.name, font.size, font.bold)
}

static GLOBAL_LANGUAGE_DATA: LazyLock<RwLock<GlobalLanguageData>> = LazyLock::new(|| {
    let mut data = GlobalLanguageData::default();
    data.init();
    RwLock::new(data)
});

pub fn get_global_language_data() -> &'static RwLock<GlobalLanguageData> {
    &GLOBAL_LANGUAGE_DATA
}

/// Force Language.ini load / re-sync (C++ `TheGlobalLanguageData->init()`).
pub fn init_global_language_data() {
    if let Ok(mut guard) = GLOBAL_LANGUAGE_DATA.write() {
        guard.init();
    }
}
