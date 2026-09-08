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
use gamelogic::helpers::TheGameLogic;

use crate::draw_group_info::get_draw_group_info;
use crate::game_text::GameText;

use super::font::{FontDesc, GameFont, get_font_library};
use super::game_window::GameFont as LegacyGameFont;
use super::ui_globals::with_ui_renderer_mut;
use super::ui_renderer::{UIRect, UIRenderer};

pub type DisplayStringHandle = Rc<RefCell<DisplayString>>;

const DEFAULT_FONT_NAME: &str = "Arial";
const DEFAULT_FONT_SIZE: i32 = 12;
const DEFAULT_FONT_BOLD: bool = false;
/// C++ W3DDisplayStringManager reclaims the render resources of strings not
/// rendered for this many frames (W3DDisplayStringManager.cpp:163; the
/// original shipped 60 — 300 keeps long-lived menu/hud strings from
/// rebuilding their layout every few seconds of invisibility).
const RENDER_RESOURCE_CLEANUP_FRAMES: u32 = 300;

/// C++ update() inspects a bounded checkpointed window of strings per call
/// instead of sweeping the whole list (W3DDisplayStringManager.cpp:167-170).
const LRU_STRINGS_PER_UPDATE: usize = 10;

/// Hotkey glyph placement, resolved once per layout rebuild: the wrapped
/// line the hotkey char falls on, its char index within that line, and the
/// measured prefix width used as its x offset. Caches the one measurement
/// the draw loop used to repeat every frame.
#[derive(Clone, Copy, Debug, PartialEq)]
struct HotkeyPlacement {
    line: usize,
    local_char: usize,
    prefix_width: i32,
}

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
    cached_line_widths: Vec<i32>,
    cached_line_x_offsets: Vec<i32>,
    cached_hotkey: Option<HotkeyPlacement>,
    last_resource_frame: u32,
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
            cached_line_widths: Vec::new(),
            cached_line_x_offsets: Vec::new(),
            cached_hotkey: None,
            last_resource_frame: 0,
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
        self.cached_line_widths.clear();
        self.cached_line_x_offsets.clear();
        self.cached_hotkey = None;
        self.last_resource_frame = 0;
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

    /// C++ `W3DDisplayString::usingResources` (W3DDisplayString.h:87): stamp
    /// the frame on which render resources were last consumed so the
    /// manager's lazy LRU can reclaim them after a period of disuse.
    pub fn using_resources(&mut self, frame: u32) {
        self.last_resource_frame = frame;
    }

    /// C++ update() resource reclaim (W3DDisplayStringManager.cpp:177-194):
    /// drop the cached layout so the next draw rebuilds it, and zero the
    /// stamp so the string is ignored by future sweeps until it renders
    /// again.
    fn free_render_resources(&mut self) {
        self.cached_lines.clear();
        self.cached_line_widths.clear();
        self.cached_line_x_offsets.clear();
        self.cached_hotkey = None;
        self.cached_size = (0, 0);
        self.dirty = true;
        self.last_resource_frame = 0;
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
        // Layout, per-line widths, centered x offsets, and hotkey placement
        // are all cached; an unchanged string redraws with no measurement
        // work at all.
        if self.cached_lines.is_empty() {
            return;
        }

        let font = self.resolve_font();
        let line_height = font.get_line_height();
        let font_name = font.desc.name.clone();
        let bold = font.desc.bold;
        let point_size = font.desc.size;
        let hotkey_placement = if self.use_hotkey {
            self.cached_hotkey
        } else {
            None
        };

        let scissor = self.clip_region.as_ref().map(|region| {
            UIRect::new(
                region.x as f32,
                region.y as f32,
                region.width as f32,
                region.height as f32,
            )
        });

        for (line_idx, (line, &x_offset)) in self
            .cached_lines
            .iter()
            .zip(self.cached_line_x_offsets.iter())
            .enumerate()
        {
            let x_line = x + x_offset;
            let y_line = y + (line_idx as i32 * line_height);
            if drop_color != 0 {
                draw_text_with_scissor(
                    renderer,
                    line,
                    x_line + x_drop,
                    y_line + y_drop,
                    point_size,
                    &font_name,
                    bold,
                    drop_color,
                    scissor,
                );
            }

            draw_text_with_scissor(
                renderer, line, x_line, y_line, point_size, &font_name, bold, color, scissor,
            );

            if let Some(place) = hotkey_placement.filter(|place| place.line == line_idx) {
                let ch = line.chars().nth(place.local_char).unwrap_or(' ');
                let hotkey_text = ch.to_string();
                // C++ renders the hotkey letter with a dedicated BOLD font
                // (`m_textRendererHotKey.Set_Font(getFont(name, size, TRUE))`,
                // W3DDisplayString.cpp:276) instead of the string's weight.
                draw_text_with_scissor(
                    renderer,
                    &hotkey_text,
                    x_line + place.prefix_width,
                    y_line,
                    font.desc.size,
                    &font.desc.name,
                    true,
                    self.hotkey_color,
                    scissor,
                );
            }
        }

        // C++ W3DDisplayString::draw stamps resource use on every render
        // (W3DDisplayString.cpp:205-207) so the manager's LRU can reclaim
        // render data from strings that stop being drawn.
        self.using_resources(TheGameLogic::get_frame());
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
        let (text, hotkey_index) = self.visible_text_with_hotkey();
        let mut lines = Vec::new();
        let mut line_widths = Vec::new();
        let mut max_width = 0;

        // C++ `W3DDisplayString::notifyTextChanged` applies the language
        // INI's `UseHardWordWrap` to every string's renderer
        // (W3DDisplayString.cpp:91-103).
        let hard_wrap = crate::global_language::get_global_language_data()
            .read()
            .map(|language| language.use_hard_wrap)
            .unwrap_or(false);

        for raw_line in text.split('\n') {
            if let Some(wrap_width) = self.word_wrap {
                self.wrap_line(
                    raw_line,
                    wrap_width,
                    &font,
                    hard_wrap,
                    &mut lines,
                    &mut line_widths,
                    &mut max_width,
                );
            } else {
                let width = font.measure_text(raw_line);
                if width > max_width {
                    max_width = width;
                }
                lines.push(raw_line.to_string());
                line_widths.push(width);
            }
        }

        // Hotkey placement (line, in-line char index, prefix width) is
        // measured once here; draw reuses it instead of re-measuring the
        // prefix every frame (C++ caches m_hotKeyPos the same way).
        let cached_hotkey = if self.use_hotkey {
            hotkey_index.and_then(|index| Self::resolve_hotkey_placement(&lines, &font, index))
        } else {
            None
        };

        // Centered x offsets are a pure function of the cached widths (C++
        // Draw_Text centering); precompute them so draw does no per-line
        // work beyond one add.
        let cached_line_x_offsets: Vec<i32> = lines
            .iter()
            .zip(&line_widths)
            .map(|(&_, &width)| {
                // C++ enters the centered builder when
                // `Centered && (WrapWidth > 0 || text contains '\n')`
                // (render2dsentence.cpp:1143) and centers each line against
                // the widest formatted line, clamped at 0
                // (render2dsentence.cpp:831-833) — not against the wrap
                // width.
                if self.word_wrap_centered && (self.word_wrap.is_some() || text.contains('\n')) {
                    ((max_width - width) / 2).max(0)
                } else {
                    0
                }
            })
            .collect();

        let height = line_height * lines.len() as i32;
        self.cached_lines = lines;
        self.cached_line_widths = line_widths;
        self.cached_line_x_offsets = cached_line_x_offsets;
        self.cached_hotkey = cached_hotkey;
        self.cached_size = (max_width, height);
        self.dirty = false;
    }

    /// Locate the line holding `hotkey_index` (a char index into the visible
    /// text) and measure its prefix once. Returns None when the index falls
    /// past the last line, mirroring the per-draw range check it replaces.
    fn resolve_hotkey_placement(
        lines: &[String],
        font: &GameFont,
        hotkey_index: usize,
    ) -> Option<HotkeyPlacement> {
        let mut char_offset = 0usize;
        for (line_idx, line) in lines.iter().enumerate() {
            let line_len = line.chars().count();
            if hotkey_index < char_offset + line_len {
                let local_char = hotkey_index - char_offset;
                let prefix: String = line.chars().take(local_char).collect();
                return Some(HotkeyPlacement {
                    line: line_idx,
                    local_char,
                    prefix_width: font.measure_text(&prefix),
                });
            }
            char_offset += line_len;
        }
        None
    }

    fn wrap_line(
        &self,
        raw_line: &str,
        wrap_width: i32,
        font: &GameFont,
        hard_wrap: bool,
        lines: &mut Vec<String>,
        line_widths: &mut Vec<i32>,
        max_width: &mut i32,
    ) {
        let mut current = String::new();
        for word in raw_line.split_whitespace() {
            // C++ `useHardWordWrap` splits a word that cannot fit a fresh
            // line at the wrap column instead of letting it overflow
            // (render2dsentence.cpp:1006, 1061-1064).
            if hard_wrap && font.measure_text(word) > wrap_width {
                let space_width = font.measure_text(" ");
                let mut line = current;
                let mut line_width = if line.is_empty() {
                    0
                } else {
                    font.measure_text(&line)
                };
                let mut word_started = false;
                for ch in word.chars() {
                    let ch_width = font.measure_text(&ch.to_string());
                    if word_started && line_width + ch_width > wrap_width {
                        *max_width = (*max_width).max(line_width);
                        lines.push(std::mem::take(&mut line));
                        line_widths.push(line_width);
                        line_width = 0;
                        word_started = false;
                    }
                    if !word_started && !line.is_empty() {
                        line.push(' ');
                        line_width += space_width;
                    }
                    line.push(ch);
                    line_width += ch_width;
                    word_started = true;
                }
                current = line;
                continue;
            }
            let candidate = if current.is_empty() {
                word.to_string()
            } else {
                format!("{} {}", current, word)
            };
            if font.measure_text(&candidate) > wrap_width && !current.is_empty() {
                let width = font.measure_text(&current);
                *max_width = (*max_width).max(width);
                lines.push(current);
                line_widths.push(width);
                current = word.to_string();
            } else {
                current = candidate;
            }
        }

        if current.is_empty() && raw_line.is_empty() {
            lines.push(String::new());
            line_widths.push(0);
            return;
        }

        if !current.is_empty() {
            let width = font.measure_text(&current);
            *max_width = (*max_width).max(width);
            lines.push(current);
            line_widths.push(width);
        }
    }
}

fn draw_text_with_scissor(
    renderer: &mut UIRenderer,
    text: &str,
    x: i32,
    y: i32,
    point_size: i32,
    font_name: &str,
    bold: bool,
    color: u32,
    scissor: Option<UIRect>,
) {
    let color = color_to_rgba(color);
    let pos = Vec2::new(x as f32, y as f32);
    if let Some(scissor) = scissor {
        let _ = renderer.draw_text_simple_named_with_scissor(
            text,
            pos,
            point_size as f32,
            color,
            font_name,
            bold,
            scissor,
        );
    } else {
        let _ =
            renderer.draw_text_simple_named(text, pos, point_size as f32, color, font_name, bold);
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
    /// C++ `m_currentCheckpoint`: resume point of the lazy LRU sweep so each
    /// update inspects a bounded window instead of the whole list
    /// (W3DDisplayStringManager.cpp:157-160, 204).
    current_checkpoint: Option<DisplayStringHandle>,
}

fn apply_draw_group_style(handle: &DisplayStringHandle, text: String) {
    let mut display = handle.borrow_mut();
    display.set_text(text);
    let info = get_draw_group_info()
        .read()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let desc = FontDesc::new(&info.font_name, info.font_size, info.font_is_bold);
    if let Ok(font) = get_font_library().get_font(&desc) {
        display.set_font(font);
    }
}

impl DisplayStringManager {
    pub fn new() -> Self {
        Self {
            strings: Vec::new(),
            group_numerals: std::array::from_fn(|_| None),
            formation_letter: None,
            default_font: None,
            current_checkpoint: None,
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

    /// Checkpointed lazy LRU ported from W3DDisplayStringManager.cpp:150-205:
    /// walk up to `LRU_STRINGS_PER_UPDATE` strings per call from the last
    /// checkpoint, freeing render resources of strings whose last render is
    /// more than `RENDER_RESOURCE_CLEANUP_FRAMES` frames ago. The frame is a
    /// parameter (C++ reads TheGameClient->getFrame()) so tests can drive
    /// the clock deterministically.
    pub fn update_to_frame(&mut self, curr_frame: u32) {
        let start = self
            .current_checkpoint
            .as_ref()
            .and_then(|checkpoint| {
                self.strings
                    .iter()
                    .position(|string| Rc::ptr_eq(string, checkpoint))
            })
            .unwrap_or(0);

        let mut index = start;
        while index < self.strings.len() && index - start < LRU_STRINGS_PER_UPDATE {
            let mut string = self.strings[index].borrow_mut();
            if string.last_resource_frame != 0
                && curr_frame.saturating_sub(string.last_resource_frame)
                    > RENDER_RESOURCE_CLEANUP_FRAMES
            {
                string.free_render_resources();
            }
            index += 1;
        }

        // Park on the first unvisited string; None restarts at the head on
        // the next call, matching the C++ checkpoint-at-list-end behavior.
        self.current_checkpoint = self.strings.get(index).cloned();
    }

    pub fn get_group_numeral_string(&mut self, numeral: i32) -> Option<DisplayStringHandle> {
        let idx = numeral.clamp(0, 9) as usize;
        if let Some(existing) = self.group_numerals[idx].as_ref() {
            apply_draw_group_style(existing, GameText::fetch(&format!("NUMBER:{idx}")));
            return Some(existing.clone());
        }

        let handle = self.new_display_string();
        apply_draw_group_style(&handle, GameText::fetch(&format!("NUMBER:{idx}")));
        self.group_numerals[idx] = Some(handle.clone());
        Some(handle)
    }

    pub fn get_formation_letter_string(&mut self) -> Option<DisplayStringHandle> {
        if let Some(existing) = self.formation_letter.as_ref() {
            apply_draw_group_style(existing, GameText::fetch("LABEL:FORMATION"));
            return Some(existing.clone());
        }

        let handle = self.new_display_string();
        apply_draw_group_style(&handle, GameText::fetch("LABEL:FORMATION"));
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
        // C++ W3DDisplayStringManager::update (W3DDisplayStringManager.cpp:
        // 150-205): lazy LRU sweep over the string list.
        self.update_to_frame(TheGameLogic::get_frame());
        Ok(())
    }

    fn reset(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        self.current_checkpoint = None;
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

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicU32, Ordering};

    use super::*;
    use crate::gui::font::{FontData, FontMetrics};

    /// FontData stub with a fixed per-char advance and a measure counter so
    /// tests can observe exactly when measurement happens.
    struct CountingFontData {
        char_width: i32,
        measures: Arc<AtomicU32>,
    }

    impl FontData for CountingFontData {
        fn get_metrics(&self) -> FontMetrics {
            FontMetrics::default()
        }

        fn measure_text(&self, text: &str) -> i32 {
            self.measures.fetch_add(1, Ordering::SeqCst);
            text.chars().count() as i32 * self.char_width
        }

        fn get_line_height(&self) -> i32 {
            14
        }

        fn supports_char(&self, _ch: char) -> bool {
            true
        }
    }

    fn counting_font(measures: Arc<AtomicU32>) -> Arc<GameFont> {
        Arc::new(GameFont {
            desc: FontDesc::new("Counting", 12, false),
            height: 14,
            font_data: Box::new(CountingFontData {
                char_width: 8,
                measures,
            }),
        })
    }

    #[test]
    fn layout_cache_measures_once_until_text_changes() {
        let measures = Arc::new(AtomicU32::new(0));
        let mut string = DisplayString::new();
        string.set_font(counting_font(measures.clone()));
        string.set_text("alpha beta gamma");
        string.set_word_wrap(100);
        string.set_word_wrap_centered(true);

        // First layout: "alpha beta" (80px) / "gamma" (40px), two 14px lines.
        let size = string.get_size();
        assert_eq!(size, (80, 28));
        let after_layout = measures.load(Ordering::SeqCst);
        assert!(after_layout > 0);

        // Unchanged text re-measures nothing (this is the gate every draw
        // runs through first).
        assert_eq!(string.get_size(), size);
        assert_eq!(measures.load(Ordering::SeqCst), after_layout);

        // Widths and centered x offsets are cached alongside the lines.
        // C++ Build_Sentence_Centered centers each line against the widest
        // formatted line (extent.X = 80 here), clamped at 0 — not against
        // the wrap width (render2dsentence.cpp:831-833).
        assert_eq!(string.cached_line_widths, vec![80, 40]);
        assert_eq!(string.cached_line_x_offsets, vec![0, 20]);

        // New text invalidates the cache and measures exactly once more.
        string.set_text("alpha beta gamma delta");
        let _ = string.get_size();
        assert!(measures.load(Ordering::SeqCst) > after_layout);
    }

    #[test]
    fn hotkey_prefix_is_measured_at_layout_not_draw() {
        let measures = Arc::new(AtomicU32::new(0));
        let mut string = DisplayString::new();
        string.set_font(counting_font(measures.clone()));
        string.set_text("Save &G&ame");
        string.set_use_hotkey(true, 0xFF00_FF00);

        // Visible text "Save Game": the hotkey 'G' sits at char 5 of line 0,
        // and its 5-char prefix measures 40px once at layout time.
        let _ = string.get_size();
        assert_eq!(
            string.cached_hotkey,
            Some(HotkeyPlacement {
                line: 0,
                local_char: 5,
                prefix_width: 40,
            })
        );
        let after_layout = measures.load(Ordering::SeqCst);
        assert!(after_layout >= 2);

        // Redrawing (and re-entering the layout gate) adds no measurements.
        assert_eq!(string.get_size(), string.cached_size);
        assert_eq!(measures.load(Ordering::SeqCst), after_layout);
    }

    #[test]
    fn lru_reclaims_stale_strings_in_checkpoint_windows() {
        let mut manager = DisplayStringManager::new();
        let mut handles = Vec::new();
        for _ in 0..12 {
            handles.push(manager.new_display_string());
        }
        // A string that laid out but never rendered: stamp 0 means "uses no
        // resources" and must be ignored by the sweep (C++ :177).
        let never_rendered = manager.new_display_string();
        for handle in &handles {
            let mut string = handle.borrow_mut();
            string.set_text("stale");
            let _ = string.get_size();
            string.using_resources(100);
        }
        {
            let mut string = never_rendered.borrow_mut();
            string.set_text("idle");
            let _ = string.get_size();
        }

        // At exactly the threshold the cache survives (C++ uses strict >).
        manager.update_to_frame(400);
        assert!(handles.iter().all(|handle| !handle.borrow().dirty));

        // Frame 401: only the checkpoint window (10 strings) is reclaimed
        // per update — never a mass sweep.
        manager.update_to_frame(401);
        for handle in handles.iter().skip(10) {
            assert!(handle.borrow().dirty, "window strings free first");
        }
        for handle in handles.iter().take(10) {
            assert!(!handle.borrow().dirty);
        }
        assert!(!never_rendered.borrow().dirty);

        // The next update resumes from the checkpoint and reclaims the rest;
        // freed strings rebuild on demand afterwards.
        manager.update_to_frame(401);
        for handle in handles.iter().take(10) {
            let mut string = handle.borrow_mut();
            assert!(string.dirty);
            assert!(string.cached_lines.is_empty());
            string.update_layout_cache();
            assert!(!string.dirty, "freed strings rebuild on demand");
        }
        assert!(!never_rendered.borrow().dirty);
    }
}
