//! Credits manager ported from C++ Credits.cpp.

use std::collections::{HashSet, VecDeque};
use std::fs;
use std::path::PathBuf;
use std::sync::{Arc, RwLock};

use game_engine::common::ini::get_global_data;
use game_engine::common::language::Language;

use crate::color::{game_get_color_components, game_make_color, Color};
use crate::global_language::get_global_language_data;
use crate::gui::display_string::get_display_string_manager;
use crate::gui::display_string::DisplayStringHandle;
use crate::gui::font::{get_font_library, FontDesc};
use crate::gui::ui_globals::with_ui_renderer;

const CREDIT_SPACE_OFFSET: i32 = 2;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CreditStyle {
    Title,
    Position,
    Normal,
    Column,
    Blank,
}

impl CreditStyle {
    fn parse(value: &str) -> Option<Self> {
        match value.trim().to_ascii_uppercase().as_str() {
            "TITLE" => Some(CreditStyle::Title),
            "MINORTITLE" => Some(CreditStyle::Position),
            "NORMAL" => Some(CreditStyle::Normal),
            "COLUMN" => Some(CreditStyle::Column),
            _ => None,
        }
    }
}

#[derive(Clone)]
pub struct CreditsLine {
    style: CreditStyle,
    text: String,
    second_text: String,
    use_second: bool,
    done: bool,
    display_string: Option<DisplayStringHandle>,
    second_display_string: Option<DisplayStringHandle>,
    pos_x: i32,
    pos_y: i32,
    height: i32,
    color: Color,
}

impl CreditsLine {
    fn new(style: CreditStyle) -> Self {
        Self {
            style,
            text: String::new(),
            second_text: String::new(),
            use_second: false,
            done: false,
            display_string: None,
            second_display_string: None,
            pos_x: 0,
            pos_y: 0,
            height: 0,
            color: game_make_color(255, 255, 255, 255),
        }
    }
}

#[derive(Clone)]
pub struct CreditsManager {
    credit_lines: Vec<CreditsLine>,
    credit_index: usize,
    displayed_lines: VecDeque<usize>,
    scroll_rate: i32,
    scroll_rate_per_frames: i32,
    scroll_down: bool,
    title_color: Color,
    position_color: Color,
    normal_color: Color,
    current_style: CreditStyle,
    is_finished: bool,
    frames_since_started: i32,
    normal_font_height: i32,
    title_font: FontDesc,
    position_font: FontDesc,
    normal_font: FontDesc,
}

impl Default for CreditsManager {
    fn default() -> Self {
        Self::new()
    }
}

impl CreditsManager {
    pub fn new() -> Self {
        Self {
            credit_lines: Vec::new(),
            credit_index: 0,
            displayed_lines: VecDeque::new(),
            scroll_rate: 1,
            scroll_rate_per_frames: 1,
            scroll_down: true,
            title_color: game_make_color(255, 255, 255, 255),
            position_color: game_make_color(255, 255, 255, 255),
            normal_color: game_make_color(255, 255, 255, 255),
            current_style: CreditStyle::Normal,
            is_finished: false,
            frames_since_started: 0,
            normal_font_height: 10,
            title_font: FontDesc::new("Arial", 18, false),
            position_font: FontDesc::new("Arial", 14, false),
            normal_font: FontDesc::new("Arial", 12, false),
        }
    }

    pub fn init(&mut self) {
        self.is_finished = false;
        self.credit_index = 0;
        self.frames_since_started = 0;
        self.displayed_lines.clear();
    }

    pub fn reset(&mut self) {
        for line in &mut self.credit_lines {
            Self::free_display_strings(line);
        }
        self.displayed_lines.clear();
        self.is_finished = false;
        self.credit_index = 0;
        self.frames_since_started = 0;
    }

    pub fn load_from_path(&mut self, path: &str) -> Result<(), String> {
        let path = self
            .resolve_credits_path(path)
            .ok_or_else(|| format!("Credits file '{}' not found in known locations", path))?;
        let content = fs::read_to_string(&path)
            .map_err(|err| format!("Failed to read credits file: {err}"))?;
        self.parse_content(&content);
        self.scroll_rate = self.scroll_rate.max(1);
        self.scroll_rate_per_frames = self.scroll_rate_per_frames.max(1);
        if let Ok(global) = get_global_language_data().read() {
            self.title_font = global.credits_title_font.clone();
            self.position_font = global.credits_position_font.clone();
            self.normal_font = global.credits_normal_font.clone();
        }
        if let Ok(font) = get_font_library().get_font(&self.normal_font) {
            self.normal_font_height = font.height;
        }
        Ok(())
    }

    pub fn update(&mut self) {
        if self.is_finished {
            return;
        }

        self.frames_since_started += 1;
        if self.frames_since_started % self.scroll_rate_per_frames != 0 {
            return;
        }

        let (display_width, display_height) = screen_size();
        let start = if self.scroll_down { 0 } else { display_height };
        let end = if self.scroll_down { display_height } else { 0 };
        let offset_start = if self.scroll_down { -1 } else { 0 };
        let offset_end = if self.scroll_down { 0 } else { 1 };
        let direction = if self.scroll_down { 1 } else { -1 };

        let mut last_height = 0;
        let mut last_y = 0;
        let mut kept = VecDeque::with_capacity(self.displayed_lines.len());
        while let Some(index) = self.displayed_lines.pop_front() {
            if let Some(line) = self.credit_lines.get_mut(index) {
                line.pos_y += self.scroll_rate * direction;
                last_height = line.height;
                last_y = line.pos_y;
                let y_test = line.pos_y + (line.height + CREDIT_SPACE_OFFSET) * offset_end;
                let remove =
                    (self.scroll_down && y_test > end) || (!self.scroll_down && y_test < end);
                if remove {
                    Self::free_display_strings(line);
                } else {
                    kept.push_back(index);
                }
            }
        }
        self.displayed_lines = kept;

        let y_test = last_y + (last_height + CREDIT_SPACE_OFFSET) * offset_start;
        if !((self.scroll_down && y_test >= start) || (!self.scroll_down && y_test <= start)) {
            return;
        }

        if self.displayed_lines.is_empty() && self.credit_index >= self.credit_lines.len() {
            self.is_finished = true;
        }

        if self.credit_index >= self.credit_lines.len() {
            return;
        }

        let line_index = self.credit_index;
        self.prepare_line_at_index(line_index, start, offset_start, display_width);
        self.displayed_lines.push_back(line_index);
        self.credit_index += 1;
    }

    pub fn draw(&self) {
        let (display_width, display_height) = screen_size();
        let height_chunk = display_height / 3;

        for index in &self.displayed_lines {
            let line = &self.credit_lines[*index];
            let perc = if line.pos_y < height_chunk || line.pos_y > height_chunk * 2 {
                if line.pos_y < 0 || line.pos_y > display_height {
                    0.0
                } else if line.pos_y < height_chunk {
                    line.pos_y as f32 / height_chunk as f32
                } else {
                    1.0 - ((line.pos_y - 2 * height_chunk) as f32 / height_chunk as f32)
                }
            } else {
                1.0
            };

            let (r, g, b, a) = game_get_color_components(line.color);
            let color = game_make_color(r, g, b, ((a as f32) * perc) as u8);
            let drop_color = game_make_color(0, 0, 0, ((a as f32) * perc) as u8);

            match line.style {
                CreditStyle::Title | CreditStyle::Position | CreditStyle::Normal => {
                    if let Some(display) = &line.display_string {
                        display
                            .borrow_mut()
                            .draw_with_drop(line.pos_x, line.pos_y, color, drop_color, 1, 1);
                    }
                }
                CreditStyle::Column => {
                    let chunk = display_width / 3;
                    if let Some(display) = &line.display_string {
                        let (width, _) = display.borrow_mut().get_size();
                        display.borrow_mut().draw_with_drop(
                            chunk - width / 2,
                            line.pos_y,
                            color,
                            drop_color,
                            1,
                            1,
                        );
                    }
                    if let Some(display) = &line.second_display_string {
                        let (width, _) = display.borrow_mut().get_size();
                        display.borrow_mut().draw_with_drop(
                            2 * chunk - width / 2,
                            line.pos_y,
                            color,
                            drop_color,
                            1,
                            1,
                        );
                    }
                }
                CreditStyle::Blank => {}
            }
        }
    }

    pub fn is_finished(&self) -> bool {
        self.is_finished
    }

    pub fn add_blank(&mut self) {
        self.credit_lines.push(CreditsLine::new(CreditStyle::Blank));
    }

    pub fn add_text(&mut self, text: &str) {
        match self.current_style {
            CreditStyle::Title | CreditStyle::Position | CreditStyle::Normal => {
                let mut line = CreditsLine::new(self.current_style);
                line.text = self.to_display_text(text);
                self.credit_lines.push(line);
            }
            CreditStyle::Column => {
                let needs_new = self
                    .credit_lines
                    .last()
                    .map_or(true, |line| line.style != CreditStyle::Column || line.done);
                if needs_new {
                    let mut line = CreditsLine::new(CreditStyle::Column);
                    line.text = self.to_display_text(text);
                    line.use_second = true;
                    self.credit_lines.push(line);
                } else {
                    let second_text = self.to_display_text(text);
                    if let Some(line) = self.credit_lines.last_mut() {
                        line.second_text = second_text;
                        line.done = true;
                    }
                }
            }
            CreditStyle::Blank => {}
        }
    }

    fn parse_content(&mut self, content: &str) {
        self.credit_lines.clear();
        self.credit_index = 0;
        self.displayed_lines.clear();
        self.current_style = CreditStyle::Normal;

        let mut in_block = false;
        for raw_line in content.lines() {
            let line = raw_line.split(';').next().unwrap_or("").trim();
            if line.is_empty() {
                continue;
            }
            if !in_block {
                if line.eq_ignore_ascii_case("Credits") {
                    in_block = true;
                }
                continue;
            }

            if line.eq_ignore_ascii_case("End") {
                break;
            }

            if line.eq_ignore_ascii_case("Blank") {
                self.add_blank();
                continue;
            }

            if let Some((key, value)) = line.split_once('=') {
                let key = key.trim();
                let value = value.trim();
                match key {
                    "ScrollRate" => self.scroll_rate = parse_i32(value).unwrap_or(self.scroll_rate),
                    "ScrollRateEveryFrames" => {
                        self.scroll_rate_per_frames =
                            parse_i32(value).unwrap_or(self.scroll_rate_per_frames)
                    }
                    "ScrollDown" => {
                        self.scroll_down = parse_bool(value).unwrap_or(self.scroll_down)
                    }
                    "TitleColor" => {
                        if let Some(color) = parse_color(value) {
                            self.title_color = color;
                        }
                    }
                    "MinorTitleColor" => {
                        if let Some(color) = parse_color(value) {
                            self.position_color = color;
                        }
                    }
                    "NormalColor" => {
                        if let Some(color) = parse_color(value) {
                            self.normal_color = color;
                        }
                    }
                    "Style" => {
                        if let Some(style) = CreditStyle::parse(value) {
                            self.current_style = style;
                        }
                    }
                    "Text" => self.add_text(value),
                    _ => {}
                }
            }
        }
    }

    fn prepare_line_at_index(
        &mut self,
        line_index: usize,
        start: i32,
        offset_start: i32,
        display_width: i32,
    ) {
        let title_color = self.title_color;
        let position_color = self.position_color;
        let normal_color = self.normal_color;
        let title_font = self.title_font.clone();
        let position_font = self.position_font.clone();
        let normal_font = self.normal_font.clone();
        let normal_font_height = self.normal_font_height;
        let Some(line) = self.credit_lines.get_mut(line_index) else {
            return;
        };
        Self::prepare_line(
            line,
            start,
            offset_start,
            display_width,
            title_color,
            position_color,
            normal_color,
            normal_font_height,
            &title_font,
            &position_font,
            &normal_font,
        );
    }

    fn prepare_line(
        line: &mut CreditsLine,
        start: i32,
        offset_start: i32,
        display_width: i32,
        title_color: Color,
        position_color: Color,
        normal_color: Color,
        normal_font_height: i32,
        title_font: &FontDesc,
        position_font: &FontDesc,
        normal_font: &FontDesc,
    ) {
        let mut display_strings = get_display_string_manager();
        match line.style {
            CreditStyle::Title => {
                line.color = title_color;
                if !line.text.is_empty() {
                    let mut display = display_strings.new_display_string();
                    let font = get_font_library().get_font(title_font).unwrap_or_else(|_| {
                        get_font_library()
                            .get_font(normal_font)
                            .expect("Font library not initialized")
                    });
                    display.borrow_mut().set_font(font);
                    display.borrow_mut().set_text(line.text.clone());
                    let (width, height) = display.borrow_mut().get_size();
                    line.height = height;
                    line.pos_x = display_width / 2 - width / 2;
                    line.pos_y = start + line.height * offset_start;
                    line.display_string = Some(display);
                }
            }
            CreditStyle::Position => {
                line.color = position_color;
                if !line.text.is_empty() {
                    let mut display = display_strings.new_display_string();
                    let font = get_font_library()
                        .get_font(position_font)
                        .unwrap_or_else(|_| {
                            get_font_library()
                                .get_font(normal_font)
                                .expect("Font library not initialized")
                        });
                    display.borrow_mut().set_font(font);
                    display.borrow_mut().set_text(line.text.clone());
                    let (width, height) = display.borrow_mut().get_size();
                    line.height = height;
                    line.pos_x = display_width / 2 - width / 2;
                    line.pos_y = start + line.height * offset_start;
                    line.display_string = Some(display);
                }
            }
            CreditStyle::Normal => {
                line.color = normal_color;
                if !line.text.is_empty() {
                    let mut display = display_strings.new_display_string();
                    let font = get_font_library()
                        .get_font(normal_font)
                        .expect("Font library not initialized");
                    display.borrow_mut().set_font(font);
                    display.borrow_mut().set_text(line.text.clone());
                    let (width, height) = display.borrow_mut().get_size();
                    line.height = height;
                    line.pos_x = display_width / 2 - width / 2;
                    line.pos_y = start + line.height * offset_start;
                    line.display_string = Some(display);
                }
            }
            CreditStyle::Column => {
                line.color = normal_color;
                if !line.text.is_empty() {
                    let mut display = display_strings.new_display_string();
                    let font = get_font_library()
                        .get_font(normal_font)
                        .expect("Font library not initialized");
                    display.borrow_mut().set_font(font);
                    display.borrow_mut().set_text(line.text.clone());
                    let (width, height) = display.borrow_mut().get_size();
                    line.height = height;
                    line.pos_x = display_width / 2 - width / 2;
                    line.pos_y = start + line.height * offset_start;
                    line.display_string = Some(display);
                }
                if line.use_second && !line.second_text.is_empty() {
                    let mut display = display_strings.new_display_string();
                    let font = get_font_library()
                        .get_font(normal_font)
                        .expect("Font library not initialized");
                    display.borrow_mut().set_font(font);
                    display.borrow_mut().set_text(line.second_text.clone());
                    let (width, height) = display.borrow_mut().get_size();
                    line.height = height;
                    line.pos_x = display_width / 2 - width / 2;
                    line.pos_y = start + line.height * offset_start;
                    line.second_display_string = Some(display);
                }
            }
            CreditStyle::Blank => {
                line.height = normal_font_height;
                line.pos_y = start + line.height * offset_start;
            }
        }
    }

    fn free_display_strings(line: &mut CreditsLine) {
        let mut manager = get_display_string_manager();
        if let Some(display) = line.display_string.take() {
            manager.free_display_string(display);
        }
        if let Some(display) = line.second_display_string.take() {
            manager.free_display_string(display);
        }
    }

    fn to_display_text(&self, text: &str) -> String {
        let trimmed = text.trim();
        if trimmed.eq_ignore_ascii_case("<BLANK>") {
            return String::new();
        }
        if trimmed.starts_with('"') && trimmed.ends_with('"') && trimmed.len() >= 2 {
            return trimmed.trim_matches('"').to_string();
        }
        if trimmed.contains(':') {
            return Language::get_localized_string(trimmed);
        }
        trimmed.to_string()
    }

    fn resolve_credits_path(&self, path: &str) -> Option<PathBuf> {
        let direct = PathBuf::from(path);
        if direct.exists() {
            return Some(direct);
        }

        let mut roots = HashSet::<PathBuf>::new();
        roots.insert(PathBuf::from("."));
        if let Ok(current) = std::env::current_dir() {
            roots.insert(current.clone());
            for ancestor in current.ancestors() {
                roots.insert(ancestor.to_path_buf());
            }
        }

        if let Some(global) = get_global_data() {
            let mod_dir = global.read().mod_dir.clone();
            if !mod_dir.trim().is_empty() {
                roots.insert(PathBuf::from(mod_dir.trim()));
            }
        }

        for root in roots {
            let candidate = root.join(path);
            if candidate.exists() {
                return Some(candidate);
            }

            for extracted in [
                root.join("windows_game/extracted_big_files/INIZH"),
                root.join("windows_game/extracted_big_files_v2/INIZH"),
            ] {
                let candidate = extracted.join(path);
                if candidate.exists() {
                    return Some(candidate);
                }
            }
        }

        None
    }
}

thread_local! {
    static THE_CREDITS: Arc<RwLock<CreditsManager>> =
        Arc::new(RwLock::new(CreditsManager::new()));
}

pub fn get_the_credits() -> Arc<RwLock<CreditsManager>> {
    THE_CREDITS.with(|credits| credits.clone())
}

fn screen_size() -> (i32, i32) {
    with_ui_renderer(|renderer| {
        let renderer = renderer.read().ok()?;
        let (w, h) = renderer.screen_size();
        Some((w as i32, h as i32))
    })
    .flatten()
    .unwrap_or((1024, 768))
}

fn parse_i32(value: &str) -> Option<i32> {
    value
        .trim()
        .trim_end_matches(';')
        .trim()
        .parse::<i32>()
        .ok()
}

fn parse_bool(value: &str) -> Option<bool> {
    let val = value
        .trim()
        .trim_end_matches(';')
        .trim()
        .to_ascii_uppercase();
    match val.as_str() {
        "YES" | "TRUE" => Some(true),
        "NO" | "FALSE" => Some(false),
        _ => None,
    }
}

fn parse_color(value: &str) -> Option<Color> {
    let mut r = None;
    let mut g = None;
    let mut b = None;
    let mut a = None;
    for part in value.split_whitespace() {
        let part = part.trim().trim_end_matches(';');
        if let Some((key, val)) = part.split_once(':') {
            if let Ok(parsed) = val.parse::<u8>() {
                match key.to_ascii_uppercase().as_str() {
                    "R" => r = Some(parsed),
                    "G" => g = Some(parsed),
                    "B" => b = Some(parsed),
                    "A" => a = Some(parsed),
                    _ => {}
                }
            }
        }
    }
    Some(game_make_color(
        r.unwrap_or(255),
        g.unwrap_or(255),
        b.unwrap_or(255),
        a.unwrap_or(255),
    ))
}
