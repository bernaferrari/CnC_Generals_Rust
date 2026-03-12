// FILE: credits.rs
//-----------------------------------------------------------------------------
//
//                       Electronic Arts Pacific.
//
//                       Confidential Information
//                Copyright (C) 2002 - All Rights Reserved
//
//-----------------------------------------------------------------------------
//
//  created:    Dec 2002
//
//  Filename:   credits.rs
//
//  author:     Chris Huybregts (original C++), Rust port
//
//  purpose:    This is where all the credit texts is going to be held.
//
//-----------------------------------------------------------------------------
//
// Faithful Rust port of:
// /GeneralsMD/Code/GameEngine/Source/GameClient/Credits.cpp
// /GeneralsMD/Code/GameEngine/Include/GameClient/Credits.h

use std::sync::{Arc, Mutex, OnceLock};
use std::collections::VecDeque;

// Re-export types from diplomacy module
use super::{Color, game_make_color};

// Constants matching C++ Credits.h lines 53
const CREDIT_SPACE_OFFSET: i32 = 2;

// Credit style enumeration (matches C++ Credits.h lines 43-51)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum CreditStyle {
    Title = 0,
    Position = 1,
    Normal = 2,
    Column = 3,
    Blank = 4,
}

impl CreditStyle {
    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "TITLE" => Some(CreditStyle::Title),
            "MINORTITLE" => Some(CreditStyle::Position),
            "NORMAL" => Some(CreditStyle::Normal),
            "COLUMN" => Some(CreditStyle::Column),
            _ => None,
        }
    }
}

// Font descriptor (matches C++ FontDesc.h)
#[derive(Debug, Clone)]
pub struct FontDesc {
    pub name: String,
    pub size: i32,
    pub bold: bool,
}

impl FontDesc {
    pub fn new(name: String, size: i32, bold: bool) -> Self {
        Self { name, size, bold }
    }
}

// Game font trait (represents C++ GameFont)
pub trait GameFont: Send + Sync {
    fn get_height(&self) -> i32;
}

// Display string trait (represents C++ DisplayString)
pub trait DisplayString: Send + Sync {
    fn set_font(&mut self, font: Arc<dyn GameFont>);
    fn set_text(&mut self, text: &str);
    fn get_size(&self) -> (i32, i32);
    fn draw(&self, x: i32, y: i32, color: Color, border_color: Color, x_drop: i32, y_drop: i32);
}

// Display string manager trait
pub trait DisplayStringManager: Send + Sync {
    fn new_display_string(&mut self) -> Option<Box<dyn DisplayString>>;
    fn free_display_string(&mut self, display_string: Option<Box<dyn DisplayString>>);
}

// Font library trait
pub trait FontLibrary: Send + Sync {
    fn get_font(&self, name: &str, size: i32, bold: bool) -> Arc<dyn GameFont>;
}

// Display trait (represents C++ Display)
pub trait Display: Send + Sync {
    fn get_width(&self) -> i32;
    fn get_height(&self) -> i32;
}

// Global language data trait
pub trait GlobalLanguageData: Send + Sync {
    fn get_credits_title_font(&self) -> FontDesc;
    fn get_credits_position_font(&self) -> FontDesc;
    fn get_credits_normal_font(&self) -> FontDesc;
    fn adjust_font_size(&self, size: i32) -> i32;
}

// Game text trait
pub trait GameText: Send + Sync {
    fn fetch(&self, key: &str) -> String;
}

// INI parser trait
pub trait INI: Send + Sync {
    fn load(&mut self, path: &str, flags: u32);
    fn get_next_quoted_ascii_string(&mut self) -> String;
}

// Credits line structure (matches C++ CreditsLine in Credits.h lines 66-85)
pub struct CreditsLine {
    // parsing variables
    pub style: CreditStyle,
    pub text: String,
    pub second_text: String,
    pub use_second: bool,
    pub done: bool,

    // drawing variables
    pub display_string: Option<Box<dyn DisplayString>>,
    pub second_display_string: Option<Box<dyn DisplayString>>,
    pub pos_x: i32,
    pub pos_y: i32,
    pub height: i32,
    pub color: Color,
}

impl CreditsLine {
    pub fn new() -> Self {
        Self {
            style: CreditStyle::Blank,
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

// Credits manager (matches C++ CreditsManager in Credits.h lines 87-131)
pub struct CreditsManager {
    credit_line_list: Vec<CreditsLine>,
    credit_line_list_it: usize,
    displayed_credit_line_list: VecDeque<usize>,

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

    // External subsystem references (set via dependency injection)
    display_string_manager: Option<Arc<Mutex<dyn DisplayStringManager>>>,
    font_library: Option<Arc<dyn FontLibrary>>,
    display: Option<Arc<dyn Display>>,
    global_language_data: Option<Arc<dyn GlobalLanguageData>>,
    game_text: Option<Arc<dyn GameText>>,
}

impl CreditsManager {
    // Constructor (matches C++ Credits.cpp lines 102-113)
    pub fn new() -> Self {
        Self {
            credit_line_list: Vec::new(),
            credit_line_list_it: 0,
            displayed_credit_line_list: VecDeque::new(),

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

            display_string_manager: None,
            font_library: None,
            display: None,
            global_language_data: None,
            game_text: None,
        }
    }

    // Dependency injection methods
    pub fn set_display_string_manager(&mut self, manager: Arc<Mutex<dyn DisplayStringManager>>) {
        self.display_string_manager = Some(manager);
    }

    pub fn set_font_library(&mut self, library: Arc<dyn FontLibrary>) {
        self.font_library = Some(library);
    }

    pub fn set_display(&mut self, display: Arc<dyn Display>) {
        self.display = Some(display);
    }

    pub fn set_global_language_data(&mut self, data: Arc<dyn GlobalLanguageData>) {
        self.global_language_data = Some(data);
    }

    pub fn set_game_text(&mut self, text: Arc<dyn GameText>) {
        self.game_text = Some(text);
    }

    // Initialize (matches C++ Credits.cpp lines 128-133)
    pub fn init(&mut self) {
        self.is_finished = false;
        self.credit_line_list_it = 0;
        self.frames_since_started = 0;
    }

    // Load from INI (matches C++ Credits.cpp lines 135-151)
    pub fn load(&mut self, _ini_path: &str) {
        // In a real implementation, would parse INI file here
        // For now, just set up defaults

        if self.scroll_rate_per_frames <= 0 {
            self.scroll_rate_per_frames = 1;
        }
        if self.scroll_rate <= 0 {
            self.scroll_rate = 1;
        }

        // Get normal font height
        if let Some(ref global_lang) = self.global_language_data {
            if let Some(ref font_lib) = self.font_library {
                let normal_font = global_lang.get_credits_normal_font();
                let adjusted_size = global_lang.adjust_font_size(normal_font.size);
                let font = font_lib.get_font(&normal_font.name, adjusted_size, normal_font.bold);
                self.normal_font_height = font.get_height();
            }
        }
    }

    // Reset (matches C++ Credits.cpp lines 153-160)
    pub fn reset(&mut self) {
        self.displayed_credit_line_list.clear();
        self.is_finished = false;
        self.credit_line_list_it = 0;
        self.frames_since_started = 0;
    }

    // Update (matches C++ Credits.cpp lines 162-328)
    pub fn update(&mut self) {
        if self.is_finished {
            return;
        }

        self.frames_since_started += 1;

        if self.frames_since_started % self.scroll_rate_per_frames != 0 {
            return;
        }

        let display = match &self.display {
            Some(d) => d,
            None => return,
        };

        let _y;
        let y_test;
        let mut last_height = 0;
        let start = if self.scroll_down { 0 } else { display.get_height() };
        let end = if self.scroll_down { display.get_height() } else { 0 };
        let offset_start_multiplyer = if self.scroll_down { -1 } else { 0 };
        let offset_end_multiplyer = if self.scroll_down { 0 } else { 1 };
        let direction_multiplyer = if self.scroll_down { 1 } else { -1 };

        // Update positions and remove off-screen items
        // (matches C++ Credits.cpp lines 180-197)
        let mut indices_to_remove = Vec::new();
        let mut temp_y = 0;
        for (idx, &line_idx) in self.displayed_credit_line_list.iter().enumerate() {
            if line_idx < self.credit_line_list.len() {
                let c_line = &mut self.credit_line_list[line_idx];
                c_line.pos_y = c_line.pos_y + (self.scroll_rate * direction_multiplyer);
                temp_y = c_line.pos_y;
                last_height = c_line.height;
                let temp_y_test = temp_y + ((last_height + CREDIT_SPACE_OFFSET) * offset_end_multiplyer);

                if (self.scroll_down && (temp_y_test > end)) || (!self.scroll_down && (temp_y_test < end)) {
                    // Free display strings
                    if let Some(ref mut dsm) = self.display_string_manager {
                        if let Ok(mut manager) = dsm.lock() {
                            manager.free_display_string(c_line.display_string.take());
                            manager.free_display_string(c_line.second_display_string.take());
                        }
                    }
                    indices_to_remove.push(idx);
                }
            }
        }

        // Remove indices in reverse order to maintain correct positions
        for &idx in indices_to_remove.iter().rev() {
            self.displayed_credit_line_list.remove(idx);
        }

        _y = temp_y + ((last_height + CREDIT_SPACE_OFFSET) * offset_start_multiplyer);
        y_test = temp_y + ((last_height + CREDIT_SPACE_OFFSET) * offset_end_multiplyer);

        // Check if it's time to add a new string (matches C++ Credits.cpp lines 201-209)
        if !((self.scroll_down && (y_test >= start)) || (!self.scroll_down && (y_test <= start))) {
            return;
        }

        if self.displayed_credit_line_list.is_empty() && self.credit_line_list_it >= self.credit_line_list.len() {
            self.is_finished = true;
        }

        if self.credit_line_list_it >= self.credit_line_list.len() {
            return;
        }

        // Add new credit line to display (matches C++ Credits.cpp lines 211-327)
        self.add_next_credit_line(start, offset_start_multiplyer);
    }

    // Helper method to add next credit line
    fn add_next_credit_line(&mut self, start: i32, offset_start_multiplyer: i32) {
        let current_idx = self.credit_line_list_it;
        if current_idx >= self.credit_line_list.len() {
            return;
        }

        let display = match &self.display {
            Some(d) => d.clone(),
            None => return,
        };

        let global_lang = match &self.global_language_data {
            Some(gl) => gl.clone(),
            None => return,
        };

        let font_lib = match &self.font_library {
            Some(fl) => fl.clone(),
            None => return,
        };

        let c_line = &mut self.credit_line_list[current_idx];

        match c_line.style {
            CreditStyle::Title => {
                // Matches C++ Credits.cpp lines 215-234
                c_line.color = self.title_color;

                if !c_line.text.is_empty() {
                    if let Some(ref mut dsm) = self.display_string_manager {
                        if let Ok(mut manager) = dsm.lock() {
                            if let Some(mut ds) = manager.new_display_string() {
                                let title_font = global_lang.get_credits_title_font();
                                let adjusted_size = global_lang.adjust_font_size(title_font.size);
                                let font = font_lib.get_font(&title_font.name, adjusted_size, title_font.bold);

                                ds.set_font(font);
                                ds.set_text(&c_line.text);
                                let (width, height) = ds.get_size();
                                c_line.height = height;
                                c_line.pos_x = display.get_width() / 2 - width / 2;
                                c_line.pos_y = start + (c_line.height * offset_start_multiplyer);
                                c_line.display_string = Some(ds);
                            }
                        }
                    }
                }
            }
            CreditStyle::Position => {
                // Matches C++ Credits.cpp lines 236-255
                c_line.color = self.position_color;

                if !c_line.text.is_empty() {
                    if let Some(ref mut dsm) = self.display_string_manager {
                        if let Ok(mut manager) = dsm.lock() {
                            if let Some(mut ds) = manager.new_display_string() {
                                let pos_font = global_lang.get_credits_position_font();
                                let adjusted_size = global_lang.adjust_font_size(pos_font.size);
                                let font = font_lib.get_font(&pos_font.name, adjusted_size, pos_font.bold);

                                ds.set_font(font);
                                ds.set_text(&c_line.text);
                                let (width, height) = ds.get_size();
                                c_line.height = height;
                                c_line.pos_x = display.get_width() / 2 - width / 2;
                                c_line.pos_y = start + (c_line.height * offset_start_multiplyer);
                                c_line.display_string = Some(ds);
                            }
                        }
                    }
                }
            }
            CreditStyle::Normal => {
                // Matches C++ Credits.cpp lines 257-276
                c_line.color = self.normal_color;

                if !c_line.text.is_empty() {
                    if let Some(ref mut dsm) = self.display_string_manager {
                        if let Ok(mut manager) = dsm.lock() {
                            if let Some(mut ds) = manager.new_display_string() {
                                let normal_font = global_lang.get_credits_normal_font();
                                let adjusted_size = global_lang.adjust_font_size(normal_font.size);
                                let font = font_lib.get_font(&normal_font.name, adjusted_size, normal_font.bold);

                                ds.set_font(font);
                                ds.set_text(&c_line.text);
                                let (width, height) = ds.get_size();
                                c_line.height = height;
                                c_line.pos_x = display.get_width() / 2 - width / 2;
                                c_line.pos_y = start + (c_line.height * offset_start_multiplyer);
                                c_line.display_string = Some(ds);
                            }
                        }
                    }
                }
            }
            CreditStyle::Column => {
                // Matches C++ Credits.cpp lines 278-313
                c_line.color = self.normal_color;

                if !c_line.text.is_empty() {
                    if let Some(ref mut dsm) = self.display_string_manager {
                        if let Ok(mut manager) = dsm.lock() {
                            if let Some(mut ds) = manager.new_display_string() {
                                let normal_font = global_lang.get_credits_normal_font();
                                let adjusted_size = global_lang.adjust_font_size(normal_font.size);
                                let font = font_lib.get_font(&normal_font.name, adjusted_size, normal_font.bold);

                                ds.set_font(font.clone());
                                ds.set_text(&c_line.text);
                                let (width, height) = ds.get_size();
                                c_line.height = height;
                                c_line.pos_x = display.get_width() / 2 - width / 2;
                                c_line.pos_y = start + (c_line.height * offset_start_multiplyer);
                                c_line.display_string = Some(ds);
                            }
                        }
                    }
                }

                if !c_line.second_text.is_empty() {
                    if let Some(ref mut dsm) = self.display_string_manager {
                        if let Ok(mut manager) = dsm.lock() {
                            if let Some(mut ds) = manager.new_display_string() {
                                let normal_font = global_lang.get_credits_normal_font();
                                let adjusted_size = global_lang.adjust_font_size(normal_font.size);
                                let font = font_lib.get_font(&normal_font.name, adjusted_size, normal_font.bold);

                                ds.set_font(font);
                                ds.set_text(&c_line.second_text);
                                let (width, height) = ds.get_size();
                                c_line.height = height;
                                c_line.pos_x = display.get_width() / 2 - width / 2;
                                c_line.pos_y = start + (c_line.height * offset_start_multiplyer);
                                c_line.second_display_string = Some(ds);
                            }
                        }
                    }
                }
            }
            CreditStyle::Blank => {
                // Matches C++ Credits.cpp lines 315-320
                c_line.height = self.normal_font_height;
                c_line.pos_y = start + (c_line.height * offset_start_multiplyer);
            }
        }

        self.displayed_credit_line_list.push_back(current_idx);
        self.credit_line_list_it += 1;
    }

    // Draw (matches C++ Credits.cpp lines 330-384)
    pub fn draw(&self) {
        let display = match &self.display {
            Some(d) => d,
            None => return,
        };

        for &line_idx in &self.displayed_credit_line_list {
            if line_idx >= self.credit_line_list.len() {
                continue;
            }

            let c_line = &self.credit_line_list[line_idx];
            let height_chunk = display.get_height() / 3;

            // Calculate fade percentage (matches C++ Credits.cpp lines 336-349)
            let perc = if c_line.pos_y < height_chunk || c_line.pos_y > height_chunk * 2 {
                if c_line.pos_y < 0 || c_line.pos_y > display.get_height() {
                    0.0
                } else if c_line.pos_y < height_chunk {
                    (c_line.pos_y as f32) / (height_chunk as f32)
                } else {
                    1.0 - ((c_line.pos_y - 2 * height_chunk) as f32) / (height_chunk as f32)
                }
            } else {
                1.0
            };

            // Apply fade to color (matches C++ Credits.cpp lines 350-353)
            let color = Color {
                r: c_line.color.r,
                g: c_line.color.g,
                b: c_line.color.b,
                a: ((c_line.color.a as f32) * perc) as u8,
            };
            let b_color = Color {
                r: 0,
                g: 0,
                b: 0,
                a: ((c_line.color.a as f32) * perc) as u8,
            };

            match c_line.style {
                CreditStyle::Title | CreditStyle::Position | CreditStyle::Normal => {
                    // Matches C++ Credits.cpp lines 356-362
                    if let Some(ref ds) = c_line.display_string {
                        ds.draw(c_line.pos_x, c_line.pos_y, color, b_color, 1, 1);
                    }
                }
                CreditStyle::Column => {
                    // Matches C++ Credits.cpp lines 364-378
                    let chunk = display.get_width() / 3;

                    if let Some(ref ds) = c_line.display_string {
                        let (width, _) = ds.get_size();
                        ds.draw(chunk - (width / 2), c_line.pos_y, color, b_color, 1, 1);
                    }

                    if let Some(ref ds) = c_line.second_display_string {
                        let (width, _) = ds.get_size();
                        ds.draw(2 * chunk - (width / 2), c_line.pos_y, color, b_color, 1, 1);
                    }
                }
                CreditStyle::Blank => {
                    // Nothing to draw for blank lines
                }
            }
        }
    }

    // Add blank line (matches C++ Credits.cpp lines 385-390)
    pub fn add_blank(&mut self) {
        let mut c_line = CreditsLine::new();
        c_line.style = CreditStyle::Blank;
        self.credit_line_list.push(c_line);
    }

    // Add text line (matches C++ Credits.cpp lines 407-448)
    pub fn add_text(&mut self, text: String) {
        let unicode_text = self.get_unicode_string(&text);

        match self.current_style {
            CreditStyle::Title | CreditStyle::Position | CreditStyle::Normal => {
                // Matches C++ Credits.cpp lines 416-420
                let mut c_line = CreditsLine::new();
                c_line.text = unicode_text;
                c_line.style = self.current_style;
                self.credit_line_list.push(c_line);
            }
            CreditStyle::Column => {
                // Matches C++ Credits.cpp lines 422-441
                if let Some(last_line) = self.credit_line_list.last_mut() {
                    if last_line.style == CreditStyle::Column && !last_line.done {
                        last_line.second_text = unicode_text;
                        last_line.done = true;
                        return;
                    }
                }

                // Create new column line
                let mut c_line = CreditsLine::new();
                c_line.text = unicode_text;
                c_line.style = CreditStyle::Column;
                c_line.use_second = true;
                self.credit_line_list.push(c_line);
            }
            _ => {
                // Should not happen
            }
        }
    }

    // Helper to convert ASCII string to Unicode (matches C++ Credits.cpp lines 454-466)
    fn get_unicode_string(&self, str: &str) -> String {
        if str == "<BLANK>" {
            return String::new();
        }

        if str.contains(':') {
            // Fetch from game text database
            if let Some(ref game_text) = self.game_text {
                return game_text.fetch(str);
            }
        }

        // Just return the string as-is (C++ would translate ASCII to Unicode)
        str.to_string()
    }

    // Check if finished (matches C++ Credits.h line 104)
    pub fn is_finished(&self) -> bool {
        self.is_finished
    }

    // Set scroll rate
    pub fn set_scroll_rate(&mut self, rate: i32) {
        self.scroll_rate = rate;
    }

    // Set scroll rate per frames
    pub fn set_scroll_rate_per_frames(&mut self, frames: i32) {
        self.scroll_rate_per_frames = frames;
    }

    // Set scroll direction
    pub fn set_scroll_down(&mut self, down: bool) {
        self.scroll_down = down;
    }

    // Set colors
    pub fn set_title_color(&mut self, color: Color) {
        self.title_color = color;
    }

    pub fn set_position_color(&mut self, color: Color) {
        self.position_color = color;
    }

    pub fn set_normal_color(&mut self, color: Color) {
        self.normal_color = color;
    }

    // Set current style
    pub fn set_current_style(&mut self, style: CreditStyle) {
        self.current_style = style;
    }
}

// Global credits manager instance (matches C++ Credits.cpp line 45)
static THE_CREDITS: OnceLock<Arc<Mutex<CreditsManager>>> = OnceLock::new();

pub fn get_the_credits() -> Arc<Mutex<CreditsManager>> {
    THE_CREDITS.get_or_init(|| Arc::new(Mutex::new(CreditsManager::new()))).clone()
}

pub fn set_the_credits(manager: Arc<Mutex<CreditsManager>>) {
    THE_CREDITS.set(manager).ok();
}
