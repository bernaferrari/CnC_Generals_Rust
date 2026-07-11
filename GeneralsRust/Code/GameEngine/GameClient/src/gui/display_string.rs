//! DisplayString system for text layout and rendering.
//!
//! Provides a C++-style DisplayString with word-wrap, hotkey highlighting,
//! and basic size measurement.

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;

use glam::Vec2;

use crate::message_stream::game_message::IRegion2D;
use crate::system::SubsystemInterface;

use super::font::{get_font_library, FontDesc, GameFont};
use super::game_window::GameFont as LegacyGameFont;
use super::ui_globals::with_ui_renderer_mut;
use super::ui_renderer::{UIRect, UIRenderer};

pub type DisplayStringHandle = Rc<RefCell<DisplayString>>;

const DEFAULT_FONT_NAME: &str = "Arial";
const DEFAULT_FONT_SIZE: i32 = 12;
const DEFAULT_FONT_BOLD: bool = false;

/// C++-style DisplayString with cached layout info.
#[derive(Clone)]
pub struct DisplayString {
    text: String,
    font: Option<Arc<GameFont>>,
    word_wrap: Option<i32>,
    word_wrap_centered: bool,
    use_hotkey: bool,
    hotkey_color: u32,
    clip_region: Option<IRegion2D>,
    cached_lines: Vec<String>,
    cached_size: (i32, i32),
    dirty: bool,
}

pub trait DisplayFontSource {
    fn to_display_font(self) -> Arc<GameFont>;
}

impl DisplayFontSource for Arc<GameFont> {
    fn to_display_font(self) -> Arc<GameFont> {
        self
    }
}

impl DisplayFontSource for &Arc<GameFont> {
    fn to_display_font(self) -> Arc<GameFont> {
        self.clone()
    }
}

impl DisplayFontSource for &GameFont {
    fn to_display_font(self) -> Arc<GameFont> {
        if let Ok(font) = get_font_library().get_font(&self.desc) {
            return font;
        }
        Arc::new(
            GameFont::new(self.desc.clone())
                .or_else(|_| GameFont::new(FontDesc::default()))
                .unwrap_or_else(|_| {
                    GameFont::new(FontDesc::new(
                        DEFAULT_FONT_NAME,
                        DEFAULT_FONT_SIZE,
                        DEFAULT_FONT_BOLD,
                    ))
                    .expect("fallback font should be constructible")
                }),
        )
    }
}

impl DisplayFontSource for &LegacyGameFont {
    fn to_display_font(self) -> Arc<GameFont> {
        let desc = self.to_font_desc();
        if let Ok(font) = get_font_library().get_font(&desc) {
            return font;
        }
        Arc::new(
            GameFont::new(desc)
                .or_else(|_| GameFont::new(FontDesc::default()))
                .unwrap_or_else(|_| {
                    GameFont::new(FontDesc::new(
                        DEFAULT_FONT_NAME,
                        DEFAULT_FONT_SIZE,
                        DEFAULT_FONT_BOLD,
                    ))
                    .expect("fallback font should be constructible")
                }),
        )
    }
}

impl Default for DisplayString {
    fn default() -> Self {
        Self::new()
    }
}

impl DisplayString {
    pub fn new() -> Self {
        Self {
            text: String::new(),
            font: None,
            word_wrap: None,
            word_wrap_centered: false,
            use_hotkey: false,
            hotkey_color: 0,
            clip_region: None,
            cached_lines: Vec::new(),
            cached_size: (0, 0),
            dirty: true,
        }
    }

    pub fn set_text(&mut self, text: impl Into<String>) {
        let text = text.into();
        if self.text != text {
            self.text = text;
            self.dirty = true;
        }
    }

    pub fn get_text(&self) -> &str {
        &self.text
    }

    pub fn get_text_length(&self) -> usize {
        self.text.chars().count()
    }

    pub fn reset(&mut self) {
        self.text.clear();
        self.font = None;
        self.word_wrap = None;
        self.word_wrap_centered = false;
        self.use_hotkey = false;
        self.hotkey_color = 0;
        self.clip_region = None;
        self.cached_lines.clear();
        self.cached_size = (0, 0);
        self.dirty = true;
    }

    pub fn set_font<F: DisplayFontSource>(&mut self, font: F) {
        self.font = Some(font.to_display_font());
        self.dirty = true;
    }

    pub fn get_font(&self) -> Option<&Arc<GameFont>> {
        self.font.as_ref()
    }

    pub fn set_word_wrap(&mut self, width: i32) {
        self.word_wrap = if width > 0 { Some(width) } else { None };
        self.dirty = true;
    }

    pub fn set_word_wrap_centered(&mut self, centered: bool) {
        self.word_wrap_centered = centered;
    }

    pub fn set_use_hotkey(&mut self, use_hotkey: bool, hotkey_color: u32) {
        self.use_hotkey = use_hotkey;
        self.hotkey_color = hotkey_color;
        self.dirty = true;
    }

    pub fn set_clip_region(&mut self, region: Option<IRegion2D>) {
        self.clip_region = region;
    }

    pub fn remove_last_char(&mut self) {
        self.text.pop();
        self.dirty = true;
    }

    pub fn append_char(&mut self, ch: char) {
        self.text.push(ch);
        self.dirty = true;
    }

    pub fn get_width(&mut self, char_pos: i32) -> i32 {
        let text = self.visible_text();
        let count = if char_pos < 0 {
            text.chars().count()
        } else {
            char_pos as usize
        };
        let prefix: String = text.chars().take(count).collect();
        let font = self.resolve_font();
        font.measure_text(&prefix)
    }

    pub fn get_size(&mut self) -> (i32, i32) {
        self.update_layout_cache();
        self.cached_size
    }

    pub fn draw(&mut self, x: i32, y: i32, color: u32, drop_color: u32) {
        self.draw_with_drop(x, y, color, drop_color, 1, 1);
    }

    pub fn draw_with_drop(
        &mut self,
        x: i32,
        y: i32,
        color: u32,
        drop_color: u32,
        x_drop: i32,
        y_drop: i32,
    ) {
        let _ = with_ui_renderer_mut(|renderer| {
            self.draw_with_renderer(renderer, x, y, color, drop_color, x_drop, y_drop);
        });
    }

    pub fn draw_with_renderer(
        &mut self,
        renderer: &mut UIRenderer,
        x: i32,
        y: i32,
        color: u32,
        drop_color: u32,
        x_drop: i32,
        y_drop: i32,
    ) {
        self.update_layout_cache();
        let lines = self.cached_lines.clone();
        if lines.is_empty() {
            return;
        }

        let font = self.resolve_font();
        let line_height = font.get_line_height();
        let mut char_offset = 0usize;
        let (_, hotkey_index) = self.visible_text_with_hotkey();
        let hotkey_index = if self.use_hotkey { hotkey_index } else { None };

        let scissor = self.clip_region.as_ref().map(|region| {
            UIRect::new(
                region.x as f32,
                region.y as f32,
                region.width as f32,
                region.height as f32,
            )
        });

        for (line_idx, line) in lines.iter().enumerate() {
            let line_width = font.measure_text(line);
            let mut x_line = x;
            if self.word_wrap_centered {
                if let Some(wrap_width) = self.word_wrap {
                    let delta = (wrap_width - line_width) / 2;
                    x_line += delta;
                }
            }

            let y_line = y + (line_idx as i32 * line_height);
            if drop_color != 0 {
                draw_text_with_scissor(
                    renderer,
                    line,
                    x_line + x_drop,
                    y_line + y_drop,
                    font.desc.size,
                    drop_color,
                    scissor,
                );
            }

            draw_text_with_scissor(
                renderer,
                line,
                x_line,
                y_line,
                font.desc.size,
                color,
                scissor,
            );

            if let Some(hotkey_idx) = hotkey_index {
                let line_start = char_offset;
                let line_end = line_start + line.chars().count();
                if hotkey_idx >= line_start && hotkey_idx < line_end {
                    let local_idx = hotkey_idx - line_start;
                    let prefix: String = line.chars().take(local_idx).collect();
                    let ch = line.chars().nth(local_idx).unwrap_or(' ');
                    let offset_x = font.measure_text(&prefix);
                    let hotkey_text = ch.to_string();
                    draw_text_with_scissor(
                        renderer,
                        &hotkey_text,
                        x_line + offset_x,
                        y_line,
                        font.desc.size,
                        self.hotkey_color,
                        scissor,
                    );
                }
            }

            char_offset += line.chars().count();
        }
    }

    fn resolve_font(&self) -> Arc<GameFont> {
        if let Some(font) = self.font.as_ref() {
            return font.clone();
        }

        let desc = FontDesc::new(DEFAULT_FONT_NAME, DEFAULT_FONT_SIZE, DEFAULT_FONT_BOLD);
        if let Ok(font) = get_font_library().get_font(&desc) {
            return font;
        }

        Arc::new(GameFont::new(desc).unwrap_or_else(|_| {
            GameFont::new(FontDesc::default()).unwrap_or_else(|_| {
                GameFont::new(FontDesc::new(
                    DEFAULT_FONT_NAME,
                    DEFAULT_FONT_SIZE,
                    DEFAULT_FONT_BOLD,
                ))
                .unwrap()
            })
        }))
    }

    fn visible_text(&self) -> String {
        self.visible_text_with_hotkey().0
    }

    fn visible_text_with_hotkey(&self) -> (String, Option<usize>) {
        if !self.use_hotkey {
            return (self.text.clone(), None);
        }

        let mut out = String::new();
        let mut hotkey_index = None;
        let mut chars = self.text.chars().peekable();
        while let Some(ch) = chars.next() {
            if ch == '&' {
                if let Some('&') = chars.peek().copied() {
                    out.push('&');
                    chars.next();
                } else if hotkey_index.is_none() {
                    hotkey_index = Some(out.chars().count());
                }
            } else {
                out.push(ch);
            }
        }

        (out, hotkey_index)
    }

    fn update_layout_cache(&mut self) {
        if !self.dirty {
            return;
        }

        let font = self.resolve_font();
        let line_height = font.get_line_height();
        let text = self.visible_text();
        let mut lines = Vec::new();
        let mut max_width = 0;

        for raw_line in text.split('\n') {
            if let Some(wrap_width) = self.word_wrap {
                self.wrap_line(raw_line, wrap_width, &font, &mut lines, &mut max_width);
            } else {
                let width = font.measure_text(raw_line);
                if width > max_width {
                    max_width = width;
                }
                lines.push(raw_line.to_string());
            }
        }

        let height = line_height * lines.len() as i32;
        self.cached_lines = lines;
        self.cached_size = (max_width, height);
        self.dirty = false;
    }

    fn wrap_line(
        &self,
        raw_line: &str,
        wrap_width: i32,
        font: &GameFont,
        lines: &mut Vec<String>,
        max_width: &mut i32,
    ) {
        let mut current = String::new();
        for word in raw_line.split_whitespace() {
            let candidate = if current.is_empty() {
                word.to_string()
            } else {
                format!("{} {}", current, word)
            };
            if font.measure_text(&candidate) > wrap_width && !current.is_empty() {
                let width = font.measure_text(&current);
                *max_width = (*max_width).max(width);
                lines.push(current);
                current = word.to_string();
            } else {
                current = candidate;
            }
        }

        if current.is_empty() && raw_line.is_empty() {
            lines.push(String::new());
            return;
        }

        if !current.is_empty() {
            let width = font.measure_text(&current);
            *max_width = (*max_width).max(width);
            lines.push(current);
        }
    }
}

fn draw_text_with_scissor(
    renderer: &mut UIRenderer,
    text: &str,
    x: i32,
    y: i32,
    font_size: i32,
    color: u32,
    scissor: Option<UIRect>,
) {
    let color = color_to_rgba(color);
    let pos = Vec2::new(x as f32, y as f32);
    if let Some(scissor) = scissor {
        let _ = renderer.draw_text_simple_with_scissor(text, pos, font_size as f32, color, scissor);
    } else {
        let _ = renderer.draw_text_simple(text, pos, font_size as f32, color);
    }
}

fn color_to_rgba(color: u32) -> [f32; 4] {
    let a = ((color >> 24) & 0xFF) as f32 / 255.0;
    let r = ((color >> 16) & 0xFF) as f32 / 255.0;
    let g = ((color >> 8) & 0xFF) as f32 / 255.0;
    let b = (color & 0xFF) as f32 / 255.0;
    [r, g, b, a]
}

/// DisplayString manager/factory.
pub struct DisplayStringManager {
    strings: Vec<DisplayStringHandle>,
    group_numerals: [Option<DisplayStringHandle>; 10],
    formation_letter: Option<DisplayStringHandle>,
    default_font: Option<Arc<GameFont>>,
}

impl DisplayStringManager {
    pub fn new() -> Self {
        Self {
            strings: Vec::new(),
            group_numerals: std::array::from_fn(|_| None),
            formation_letter: None,
            default_font: None,
        }
    }

    pub fn set_default_font(&mut self, font: Arc<GameFont>) {
        self.default_font = Some(font);
    }

    pub fn new_display_string(&mut self) -> DisplayStringHandle {
        let mut display_string = DisplayString::new();
        if let Some(font) = self.default_font.clone() {
            display_string.set_font(font);
        }
        let handle = Rc::new(RefCell::new(display_string));
        self.strings.push(handle.clone());
        handle
    }

    pub fn free_display_string(&mut self, handle: DisplayStringHandle) {
        self.strings.retain(|entry| !Rc::ptr_eq(entry, &handle));
    }

    pub fn get_group_numeral_string(&mut self, numeral: i32) -> Option<DisplayStringHandle> {
        let idx = numeral.clamp(0, 9) as usize;
        if let Some(existing) = self.group_numerals[idx].as_ref() {
            return Some(existing.clone());
        }

        let handle = self.new_display_string();
        handle.borrow_mut().set_text(idx.to_string());
        self.group_numerals[idx] = Some(handle.clone());
        Some(handle)
    }

    pub fn get_formation_letter_string(&mut self) -> Option<DisplayStringHandle> {
        if let Some(existing) = self.formation_letter.as_ref() {
            return Some(existing.clone());
        }

        let handle = self.new_display_string();
        handle.borrow_mut().set_text("F");
        self.formation_letter = Some(handle.clone());
        Some(handle)
    }
}

impl Default for DisplayStringManager {
    fn default() -> Self {
        Self::new()
    }
}

impl SubsystemInterface for DisplayStringManager {
    fn init(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        if self.default_font.is_none() {
            let desc = FontDesc::new(DEFAULT_FONT_NAME, DEFAULT_FONT_SIZE, DEFAULT_FONT_BOLD);
            if let Ok(font) = get_font_library().get_font(&desc) {
                self.default_font = Some(font);
            }
        }
        Ok(())
    }

    fn update(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        Ok(())
    }

    fn reset(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        for string in &self.strings {
            string.borrow_mut().reset();
        }
        Ok(())
    }
}

thread_local! {
    static DISPLAY_STRING_MANAGER: RefCell<DisplayStringManager> =
        RefCell::new(DisplayStringManager::new());
}

pub struct DisplayStringManagerAccess;

impl DisplayStringManagerAccess {
    pub fn set_default_font(&mut self, font: Arc<GameFont>) {
        DISPLAY_STRING_MANAGER.with(|manager| manager.borrow_mut().set_default_font(font));
    }

    pub fn new_display_string(&mut self) -> DisplayStringHandle {
        DISPLAY_STRING_MANAGER.with(|manager| manager.borrow_mut().new_display_string())
    }

    pub fn free_display_string(&mut self, handle: DisplayStringHandle) {
        DISPLAY_STRING_MANAGER.with(|manager| manager.borrow_mut().free_display_string(handle));
    }

    pub fn get_group_numeral_string(&mut self, numeral: i32) -> Option<DisplayStringHandle> {
        DISPLAY_STRING_MANAGER
            .with(|manager| manager.borrow_mut().get_group_numeral_string(numeral))
    }

    pub fn get_formation_letter_string(&mut self) -> Option<DisplayStringHandle> {
        DISPLAY_STRING_MANAGER.with(|manager| manager.borrow_mut().get_formation_letter_string())
    }

    pub fn init(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        DISPLAY_STRING_MANAGER.with(|manager| manager.borrow_mut().init())
    }

    pub fn reset(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        DISPLAY_STRING_MANAGER.with(|manager| manager.borrow_mut().reset())
    }

    pub fn update(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        DISPLAY_STRING_MANAGER.with(|manager| manager.borrow_mut().update())
    }
}

pub fn get_display_string_manager() -> DisplayStringManagerAccess {
    DisplayStringManagerAccess
}
