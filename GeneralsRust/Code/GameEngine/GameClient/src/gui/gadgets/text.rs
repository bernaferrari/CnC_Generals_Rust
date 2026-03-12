//! Text Gadgets Implementation
//!
//! This module provides text-based UI controls including:
//! - **StaticText**: Read-only text display with formatting and alignment
//! - **TextEntry**: Interactive text input fields with validation
//!
//! # Features
//!
//! - Multiple text alignment options (left, center, right, vertical centering)
//! - Input validation (numeric-only, alphanumeric, ASCII-only, custom)
//! - Password/secret text masking
//! - Multi-language input method support (IME)
//! - Comprehensive keyboard navigation
//! - Custom fonts and text colors
//! - Real-time text change notifications
//!
//! # Examples
//!
//! ## Static Text
//!
//! ```rust
//! use game_client::gui::gadgets::text::*;
//!
//! // Create centered title text
//! let title = StaticText::new(1, 0, 10, 400, 30)
//!     .with_text("Game Settings")
//!     .with_alignment(TextAlignment::Center, VerticalAlignment::Center);
//!
//! // Create left-aligned description with margin
//! let desc = StaticText::new(2, 20, 50, 360, 60)
//!     .with_text("Configure your game preferences below.")
//!     .with_alignment(TextAlignment::Left, VerticalAlignment::Top)
//!     .with_margins(10, 5);
//! ```
//!
//! ## Text Entry
//!
//! ```rust
//! // Create a username field
//! let username = TextEntry::new(3, 20, 100, 200, 25)
//!     .with_placeholder("Enter username...")
//!     .with_max_length(32)
//!     .with_validation(ValidationMode::AlphanumericOnly);
//!
//! // Create a password field
//! let password = TextEntry::new(4, 20, 130, 200, 25)
//!     .with_placeholder("Password")
//!     .as_password()
//!     .with_max_length(64);
//!
//! // Create a numeric input
//! let port = TextEntry::new(5, 20, 160, 100, 25)
//!     .with_placeholder("Port")
//!     .with_validation(ValidationMode::NumericOnly)
//!     .with_max_length(5);
//! ```

use super::*;
use std::collections::VecDeque;

/// Text alignment options
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextAlignment {
    Left,
    Center,
    Right,
}

/// Vertical alignment options
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VerticalAlignment {
    Top,
    Center,
    Bottom,
}

/// Input validation modes for text entry
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ValidationMode {
    /// Accept all characters
    None,
    /// Only numeric characters (0-9)
    NumericOnly,
    /// Only alphanumeric characters (a-z, A-Z, 0-9)
    AlphanumericOnly,
    /// Only ASCII characters
    AsciiOnly,
}

/// Configuration for text rendering and behavior
#[derive(Debug, Clone)]
pub struct TextConfig {
    pub alignment: TextAlignment,
    pub vertical_alignment: VerticalAlignment,
    pub left_margin: u32,
    pub top_margin: u32,
    pub font_size: u32,
    pub line_spacing: f32,
    pub word_wrap: bool,
}

impl Default for TextConfig {
    fn default() -> Self {
        Self {
            alignment: TextAlignment::Left,
            vertical_alignment: VerticalAlignment::Top,
            left_margin: 0,
            top_margin: 0,
            font_size: 12,
            line_spacing: 1.2,
            word_wrap: false,
        }
    }
}

// ============================================================================
// Static Text Implementation
// ============================================================================

/// Static text display gadget for labels and information
pub struct StaticText {
    // Base gadget properties
    id: GadgetId,
    bounds: Rect,
    state: GadgetState,
    enabled: bool,
    visible: bool,
    focused: bool,

    // Text content and configuration
    text: String,
    config: TextConfig,

    // Visual customization
    text_color: Option<Color>,
    background_color: Option<Color>,

    // Layout cache (would be computed during rendering)
    cached_lines: Vec<String>,
    cache_dirty: bool,
}

impl StaticText {
    /// Create a new static text gadget
    pub fn new(id: GadgetId, x: i32, y: i32, width: u32, height: u32) -> Self {
        Self {
            id,
            bounds: Rect::new(x, y, width, height),
            state: GadgetState::Normal,
            enabled: true,
            visible: true,
            focused: false,

            text: String::new(),
            config: TextConfig::default(),

            text_color: None,
            background_color: None,

            cached_lines: Vec::new(),
            cache_dirty: true,
        }
    }

    /// Set the text content
    pub fn with_text<S: Into<String>>(mut self, text: S) -> Self {
        self.text = text.into();
        self.cache_dirty = true;
        self
    }

    /// Set text content (mutable)
    pub fn set_text<S: Into<String>>(&mut self, text: S) {
        self.text = text.into();
        self.cache_dirty = true;
    }

    /// Get the text content
    pub fn text(&self) -> &str {
        &self.text
    }

    pub fn config(&self) -> &TextConfig {
        &self.config
    }

    /// Set text alignment
    pub fn with_alignment(
        mut self,
        horizontal: TextAlignment,
        vertical: VerticalAlignment,
    ) -> Self {
        self.config.alignment = horizontal;
        self.config.vertical_alignment = vertical;
        self.cache_dirty = true;
        self
    }

    /// Set alignment (mutable)
    pub fn set_alignment(&mut self, horizontal: TextAlignment, vertical: VerticalAlignment) {
        self.config.alignment = horizontal;
        self.config.vertical_alignment = vertical;
        self.cache_dirty = true;
    }

    /// Set text margins
    pub fn with_margins(mut self, left: u32, top: u32) -> Self {
        self.config.left_margin = left;
        self.config.top_margin = top;
        self.cache_dirty = true;
        self
    }

    /// Set margins (mutable)
    pub fn set_margins(&mut self, left: u32, top: u32) {
        self.config.left_margin = left;
        self.config.top_margin = top;
        self.cache_dirty = true;
    }

    /// Enable word wrapping
    pub fn with_word_wrap(mut self, enabled: bool) -> Self {
        self.config.word_wrap = enabled;
        self.cache_dirty = true;
        self
    }

    /// Set word wrap (mutable)
    pub fn set_word_wrap(&mut self, enabled: bool) {
        self.config.word_wrap = enabled;
        self.cache_dirty = true;
    }

    /// Set font size
    pub fn with_font_size(mut self, size: u32) -> Self {
        self.config.font_size = size;
        self.cache_dirty = true;
        self
    }

    /// Set font size (mutable)
    pub fn set_font_size(&mut self, size: u32) {
        self.config.font_size = size;
        self.cache_dirty = true;
    }

    /// Set line spacing multiplier
    pub fn with_line_spacing(mut self, spacing: f32) -> Self {
        self.config.line_spacing = spacing;
        self.cache_dirty = true;
        self
    }

    /// Set line spacing (mutable)
    pub fn set_line_spacing(&mut self, spacing: f32) {
        self.config.line_spacing = spacing;
        self.cache_dirty = true;
    }

    /// Set text color
    pub fn with_text_color(mut self, color: Color) -> Self {
        self.text_color = Some(color);
        self
    }

    /// Set text color (mutable)
    pub fn set_text_color(&mut self, color: Option<Color>) {
        self.text_color = color;
    }

    /// Set background color
    pub fn with_background_color(mut self, color: Color) -> Self {
        self.background_color = Some(color);
        self
    }

    /// Set background color (mutable)
    pub fn set_background_color(&mut self, color: Option<Color>) {
        self.background_color = color;
    }

    /// Update cached text layout (would compute actual text positioning)
    fn update_cache(&mut self) {
        if !self.cache_dirty {
            return;
        }

        // In a real implementation, this would:
        // 1. Measure text with current font
        // 2. Apply word wrapping if enabled
        // 3. Split into lines that fit within bounds
        // 4. Calculate positioning based on alignment

        // Simplified implementation for demonstration
        if self.config.word_wrap {
            // Simple word wrap simulation
            let words: Vec<&str> = self.text.split_whitespace().collect();
            self.cached_lines.clear();

            let mut current_line = String::new();
            let max_line_length = ((self
                .bounds
                .width
                .saturating_sub(self.config.left_margin * 2))
                / 8) as usize; // Rough character estimate

            for word in words {
                if current_line.len() + word.len() + 1 > max_line_length && !current_line.is_empty()
                {
                    self.cached_lines.push(current_line);
                    current_line = word.to_string();
                } else {
                    if !current_line.is_empty() {
                        current_line.push(' ');
                    }
                    current_line.push_str(word);
                }
            }

            if !current_line.is_empty() {
                self.cached_lines.push(current_line);
            }
        } else {
            // No word wrap - split by newlines only
            self.cached_lines = self.text.lines().map(|s| s.to_string()).collect();
        }

        self.cache_dirty = false;
    }

    /// Get the effective text color
    fn get_text_color(&self, theme: &GadgetTheme) -> Color {
        self.text_color.unwrap_or_else(|| {
            if self.enabled {
                theme.text_color
            } else {
                theme.disabled_text_color
            }
        })
    }
}

impl Gadget for StaticText {
    fn id(&self) -> GadgetId {
        self.id
    }

    fn bounds(&self) -> Rect {
        self.bounds
    }

    fn set_position(&mut self, x: i32, y: i32) {
        self.bounds.x = x;
        self.bounds.y = y;
    }

    fn set_size(&mut self, width: u32, height: u32) {
        self.bounds.width = width;
        self.bounds.height = height;
        self.cache_dirty = true;
    }

    fn state(&self) -> GadgetState {
        self.state
    }

    fn is_enabled(&self) -> bool {
        self.enabled
    }

    fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }

    fn is_visible(&self) -> bool {
        self.visible
    }

    fn set_visible(&mut self, visible: bool) {
        self.visible = visible;
    }

    fn can_focus(&self) -> bool {
        false // Static text typically doesn't receive focus
    }

    fn has_focus(&self) -> bool {
        self.focused
    }

    fn set_focus(&mut self, focused: bool) {
        self.focused = focused;
    }

    fn handle_input(&mut self, event: &InputEvent) -> Vec<GadgetMessage> {
        // Static text handles keyboard navigation but doesn't process other input
        match event {
            InputEvent::KeyDown { key, .. } if self.focused => match key {
                KeyCode::Tab | KeyCode::Right | KeyCode::Down => vec![GadgetMessage::Custom {
                    gadget_id: self.id,
                    data: "tab_next".to_string(),
                }],
                KeyCode::Left | KeyCode::Up => vec![GadgetMessage::Custom {
                    gadget_id: self.id,
                    data: "tab_prev".to_string(),
                }],
                _ => Vec::new(),
            },
            _ => Vec::new(),
        }
    }

    fn update(&mut self, _delta_time: f32) {
        self.update_cache();
    }

    fn render(&self, theme: &GadgetTheme) {
        // Placeholder rendering code
        let text_color = self.get_text_color(theme);

        println!(
            "Rendering static text {} at ({}, {}) {}x{}",
            self.id, self.bounds.x, self.bounds.y, self.bounds.width, self.bounds.height
        );

        if let Some(bg_color) = self.background_color {
            println!("  Background: {:?}", bg_color);
        }

        println!("  Text color: {:?}", text_color);
        println!("  Font size: {}", self.config.font_size);
        println!(
            "  Alignment: {:?} / {:?}",
            self.config.alignment, self.config.vertical_alignment
        );
        println!(
            "  Margins: left={}, top={}",
            self.config.left_margin, self.config.top_margin
        );

        for (i, line) in self.cached_lines.iter().enumerate() {
            println!("  Line {}: '{}'", i, line);
        }
    }

    #[allow(unused_variables)]
    fn handle_tab(&mut self, direction: TabDirection) -> bool {
        // Static text participates in tab navigation only if it can be focused
        // (which is false by default, but could be configured)
        self.can_focus()
    }
}

// ============================================================================
// Text Entry Implementation
// ============================================================================

/// Callback for text entry events
pub type TextEntryCallback = Box<dyn Fn(GadgetId, &str) + Send + Sync>;

/// Interactive text input field with validation and editing capabilities
pub struct TextEntry {
    // Base gadget properties
    id: GadgetId,
    bounds: Rect,
    state: GadgetState,
    enabled: bool,
    visible: bool,
    focused: bool,

    // Text content
    text: String,
    placeholder: String,
    displayed_text: String, // For password masking

    // Input configuration
    max_length: usize,
    validation_mode: ValidationMode,
    is_password: bool,
    is_multiline: bool,

    // Cursor and selection
    cursor_position: usize,
    selection_start: Option<usize>,
    selection_end: Option<usize>,

    // Visual configuration
    config: TextConfig,
    text_color: Option<Color>,
    background_color: Option<Color>,
    border_color: Option<Color>,
    cursor_color: Color,
    selection_color: Color,

    // Scrolling for long text
    scroll_offset: usize,
    visible_from_end: bool, // Show end of text when true

    // Event callbacks
    change_callback: Option<TextEntryCallback>,
    submit_callback: Option<TextEntryCallback>,

    // Input handling
    repeat_key: Option<KeyCode>,
    repeat_timer: f32,
    repeat_delay: f32,
    repeat_rate: f32,

    // Undo/redo support
    history: VecDeque<String>,
    history_index: usize,
    max_history: usize,

    // Input method support (placeholder for IME)
    ime_composition: String,
    ime_cursor: usize,
}

impl TextEntry {
    /// Create a new text entry field
    pub fn new(id: GadgetId, x: i32, y: i32, width: u32, height: u32) -> Self {
        let mut entry = Self {
            id,
            bounds: Rect::new(x, y, width, height),
            state: GadgetState::Normal,
            enabled: true,
            visible: true,
            focused: false,

            text: String::new(),
            placeholder: String::new(),
            displayed_text: String::new(),

            max_length: 256,
            validation_mode: ValidationMode::None,
            is_password: false,
            is_multiline: false,

            cursor_position: 0,
            selection_start: None,
            selection_end: None,

            config: TextConfig::default(),
            text_color: None,
            background_color: None,
            border_color: None,
            cursor_color: Color::BLACK,
            selection_color: Color::rgba(0, 120, 215, 128),

            scroll_offset: 0,
            visible_from_end: false,

            change_callback: None,
            submit_callback: None,

            repeat_key: None,
            repeat_timer: 0.0,
            repeat_delay: 0.5,
            repeat_rate: 0.05,

            history: VecDeque::new(),
            history_index: 0,
            max_history: 50,

            ime_composition: String::new(),
            ime_cursor: 0,
        };

        entry.update_displayed_text();
        entry
    }

    /// Set placeholder text shown when field is empty
    pub fn with_placeholder<S: Into<String>>(mut self, placeholder: S) -> Self {
        self.placeholder = placeholder.into();
        self
    }

    /// Set placeholder text (mutable)
    pub fn set_placeholder<S: Into<String>>(&mut self, placeholder: S) {
        self.placeholder = placeholder.into();
    }

    /// Get placeholder text
    pub fn placeholder(&self) -> &str {
        &self.placeholder
    }

    /// Set maximum text length
    pub fn with_max_length(mut self, max_length: usize) -> Self {
        self.max_length = max_length;
        self
    }

    /// Set max length (mutable)
    pub fn set_max_length(&mut self, max_length: usize) {
        self.max_length = max_length;
        if self.text.len() > max_length {
            self.text.truncate(max_length);
            self.cursor_position = self.cursor_position.min(self.text.len());
            self.update_displayed_text();
        }
    }

    /// Set input validation mode
    pub fn with_validation(mut self, mode: ValidationMode) -> Self {
        self.validation_mode = mode;
        self
    }

    /// Set validation mode (mutable)
    pub fn set_validation(&mut self, mode: ValidationMode) {
        self.validation_mode = mode;
    }

    /// Configure as password field (mask input)
    pub fn as_password(mut self) -> Self {
        self.is_password = true;
        self.update_displayed_text();
        self
    }

    /// Set password mode (mutable)
    pub fn set_password(&mut self, is_password: bool) {
        self.is_password = is_password;
        self.update_displayed_text();
    }

    /// Check if this is a password field
    pub fn is_password(&self) -> bool {
        self.is_password
    }

    /// Enable multiline input
    pub fn with_multiline(mut self, multiline: bool) -> Self {
        self.is_multiline = multiline;
        self
    }

    /// Set multiline mode (mutable)
    pub fn set_multiline(&mut self, multiline: bool) {
        self.is_multiline = multiline;
    }

    /// Check if multiline mode is enabled
    pub fn is_multiline(&self) -> bool {
        self.is_multiline
    }

    /// Set the text content
    pub fn with_text<S: Into<String>>(mut self, text: S) -> Self {
        self.set_text(text);
        self
    }

    /// Set text content (mutable)
    pub fn set_text<S: Into<String>>(&mut self, text: S) {
        let new_text = text.into();

        // Apply length limit
        if new_text.len() > self.max_length {
            self.text = new_text[..self.max_length].to_string();
        } else {
            self.text = new_text;
        }

        // Validate the text
        self.text = self.validate_text(&self.text);

        // Update cursor position
        self.cursor_position = self.text.len();
        self.clear_selection();

        // Add to history
        self.add_to_history();

        // Update display
        self.update_displayed_text();

        // Trigger change callback
        self.trigger_change_callback();
    }

    /// Get the text content
    pub fn text(&self) -> &str {
        &self.text
    }

    /// Get the visible text (masked when password mode is enabled).
    pub fn displayed_text(&self) -> &str {
        &self.displayed_text
    }

    /// Get the cursor position in bytes within the visible text.
    pub fn cursor_position(&self) -> usize {
        self.cursor_position
    }

    /// Get current IME composition text.
    pub fn ime_composition(&self) -> &str {
        &self.ime_composition
    }

    /// Get current IME composition cursor offset.
    pub fn ime_cursor(&self) -> usize {
        self.ime_cursor
    }

    /// Set text change callback
    pub fn with_change_callback(mut self, callback: TextEntryCallback) -> Self {
        self.change_callback = Some(callback);
        self
    }

    /// Set change callback (mutable)
    pub fn set_change_callback(&mut self, callback: TextEntryCallback) {
        self.change_callback = Some(callback);
    }

    /// Set submit callback (triggered on Enter)
    pub fn with_submit_callback(mut self, callback: TextEntryCallback) -> Self {
        self.submit_callback = Some(callback);
        self
    }

    /// Set submit callback (mutable)
    pub fn set_submit_callback(&mut self, callback: TextEntryCallback) {
        self.submit_callback = Some(callback);
    }

    /// Set cursor and selection colors
    pub fn with_cursor_colors(mut self, cursor: Color, selection: Color) -> Self {
        self.cursor_color = cursor;
        self.selection_color = selection;
        self
    }

    /// Set text color
    pub fn with_text_color(mut self, color: Color) -> Self {
        self.text_color = Some(color);
        self
    }

    /// Set background color
    pub fn with_background_color(mut self, color: Color) -> Self {
        self.background_color = Some(color);
        self
    }

    /// Set border color
    pub fn with_border_color(mut self, color: Color) -> Self {
        self.border_color = Some(color);
        self
    }

    /// Clear all text
    pub fn clear(&mut self) {
        self.text.clear();
        self.cursor_position = 0;
        self.clear_selection();
        self.scroll_offset = 0;
        self.update_displayed_text();
        self.add_to_history();
        self.trigger_change_callback();
    }

    /// Select all text
    pub fn select_all(&mut self) {
        if !self.text.is_empty() {
            self.selection_start = Some(0);
            self.selection_end = Some(self.text.len());
        }
    }

    /// Clear selection
    pub fn clear_selection(&mut self) {
        self.selection_start = None;
        self.selection_end = None;
    }

    /// Check if there's an active selection
    pub fn has_selection(&self) -> bool {
        self.selection_start.is_some() && self.selection_end.is_some()
    }

    /// Get selected text
    pub fn selected_text(&self) -> Option<String> {
        if let (Some(start), Some(end)) = (self.selection_start, self.selection_end) {
            let (start, end) = if start <= end {
                (start, end)
            } else {
                (end, start)
            };
            Some(self.text[start..end].to_string())
        } else {
            None
        }
    }

    /// Cut selected text to clipboard
    pub fn cut(&mut self) -> Option<String> {
        if let Some(text) = self.selected_text() {
            self.delete_selection();
            Some(text)
        } else {
            None
        }
    }

    /// Copy selected text to clipboard
    pub fn copy(&self) -> Option<String> {
        self.selected_text()
    }

    /// Paste text from clipboard
    pub fn paste(&mut self, text: &str) {
        self.delete_selection();
        self.insert_text(text);
    }

    /// Validate text according to current validation mode
    fn validate_text(&self, text: &str) -> String {
        match self.validation_mode {
            ValidationMode::None => text.to_string(),
            ValidationMode::NumericOnly => text.chars().filter(|c| c.is_ascii_digit()).collect(),
            ValidationMode::AlphanumericOnly => {
                text.chars().filter(|c| c.is_alphanumeric()).collect()
            }
            ValidationMode::AsciiOnly => text.chars().filter(|c| c.is_ascii()).collect(),
        }
    }

    /// Insert text at cursor position
    fn insert_text(&mut self, text: &str) {
        let validated = self.validate_text(text);
        if validated.is_empty() {
            return;
        }

        // Check length limit
        if self.text.len() + validated.len() > self.max_length {
            let remaining = self.max_length.saturating_sub(self.text.len());
            if remaining == 0 {
                return;
            }
            let truncated = &validated[..remaining.min(validated.len())];
            self.text.insert_str(self.cursor_position, truncated);
            self.cursor_position += truncated.len();
        } else {
            self.text.insert_str(self.cursor_position, &validated);
            self.cursor_position += validated.len();
        }

        self.update_displayed_text();
        self.add_to_history();
        self.trigger_change_callback();
    }

    /// Insert a single character
    fn insert_char(&mut self, ch: char) {
        if self.text.len() >= self.max_length {
            return;
        }

        let ch_str = ch.to_string();
        if self.validate_text(&ch_str) == ch_str {
            self.text.insert(self.cursor_position, ch);
            self.cursor_position += 1;
            self.update_displayed_text();
            self.add_to_history();
            self.trigger_change_callback();
        }
    }

    /// Delete character before cursor (backspace)
    fn delete_before_cursor(&mut self) {
        if self.cursor_position > 0 {
            self.text.remove(self.cursor_position - 1);
            self.cursor_position -= 1;
            self.update_displayed_text();
            self.add_to_history();
            self.trigger_change_callback();
        }
    }

    /// Delete character after cursor (delete)
    fn delete_after_cursor(&mut self) {
        if self.cursor_position < self.text.len() {
            self.text.remove(self.cursor_position);
            self.update_displayed_text();
            self.add_to_history();
            self.trigger_change_callback();
        }
    }

    /// Delete selected text
    fn delete_selection(&mut self) {
        if let (Some(start), Some(end)) = (self.selection_start, self.selection_end) {
            let (start, end) = if start <= end {
                (start, end)
            } else {
                (end, start)
            };
            self.text.drain(start..end);
            self.cursor_position = start;
            self.clear_selection();
            self.update_displayed_text();
            self.add_to_history();
            self.trigger_change_callback();
        }
    }

    /// Move cursor left
    fn move_cursor_left(&mut self, select: bool) {
        if select && !self.has_selection() {
            self.selection_start = Some(self.cursor_position);
        }

        if self.cursor_position > 0 {
            self.cursor_position -= 1;
        }

        if select {
            self.selection_end = Some(self.cursor_position);
        } else {
            self.clear_selection();
        }
    }

    /// Move cursor right
    fn move_cursor_right(&mut self, select: bool) {
        if select && !self.has_selection() {
            self.selection_start = Some(self.cursor_position);
        }

        if self.cursor_position < self.text.len() {
            self.cursor_position += 1;
        }

        if select {
            self.selection_end = Some(self.cursor_position);
        } else {
            self.clear_selection();
        }
    }

    /// Move cursor to start
    fn move_cursor_home(&mut self, select: bool) {
        if select && !self.has_selection() {
            self.selection_start = Some(self.cursor_position);
        }

        self.cursor_position = 0;

        if select {
            self.selection_end = Some(self.cursor_position);
        } else {
            self.clear_selection();
        }
    }

    /// Move cursor to end
    fn move_cursor_end(&mut self, select: bool) {
        if select && !self.has_selection() {
            self.selection_start = Some(self.cursor_position);
        }

        self.cursor_position = self.text.len();

        if select {
            self.selection_end = Some(self.cursor_position);
        } else {
            self.clear_selection();
        }
    }

    /// Update displayed text (for password masking)
    fn update_displayed_text(&mut self) {
        if self.is_password {
            self.displayed_text = "*".repeat(self.text.len());
        } else {
            self.displayed_text = self.text.clone();
        }
    }

    /// Add current text to undo history
    fn add_to_history(&mut self) {
        if let Some(last) = self.history.back() {
            if last == &self.text {
                return; // Don't add duplicate entries
            }
        }

        if self.history.len() >= self.max_history {
            self.history.pop_front();
        }

        self.history.push_back(self.text.clone());
        self.history_index = self.history.len();
    }

    /// Undo last change
    pub fn undo(&mut self) -> bool {
        if self.history_index > 0 {
            self.history_index -= 1;
            if let Some(text) = self.history.get(self.history_index) {
                self.text = text.clone();
                self.cursor_position = self.text.len();
                self.clear_selection();
                self.update_displayed_text();
                self.trigger_change_callback();
                return true;
            }
        }
        false
    }

    /// Redo last undone change
    pub fn redo(&mut self) -> bool {
        if self.history_index < self.history.len() - 1 {
            self.history_index += 1;
            if let Some(text) = self.history.get(self.history_index) {
                self.text = text.clone();
                self.cursor_position = self.text.len();
                self.clear_selection();
                self.update_displayed_text();
                self.trigger_change_callback();
                return true;
            }
        }
        false
    }

    /// Trigger change callback
    fn trigger_change_callback(&self) {
        if let Some(ref callback) = self.change_callback {
            callback(self.id, &self.text);
        }
    }

    /// Trigger submit callback
    fn trigger_submit_callback(&self) {
        if let Some(ref callback) = self.submit_callback {
            callback(self.id, &self.text);
        }
    }

    /// Get the effective text color
    fn get_text_color(&self, theme: &GadgetTheme) -> Color {
        self.text_color.unwrap_or_else(|| {
            if self.enabled {
                theme.text_color
            } else {
                theme.disabled_text_color
            }
        })
    }
}

impl Gadget for TextEntry {
    fn id(&self) -> GadgetId {
        self.id
    }

    fn bounds(&self) -> Rect {
        self.bounds
    }

    fn set_position(&mut self, x: i32, y: i32) {
        self.bounds.x = x;
        self.bounds.y = y;
    }

    fn set_size(&mut self, width: u32, height: u32) {
        self.bounds.width = width;
        self.bounds.height = height;
    }

    fn state(&self) -> GadgetState {
        self.state
    }

    fn is_enabled(&self) -> bool {
        self.enabled
    }

    fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
        if !enabled {
            self.state = GadgetState::Disabled;
        }
    }

    fn is_visible(&self) -> bool {
        self.visible
    }

    fn set_visible(&mut self, visible: bool) {
        self.visible = visible;
    }

    fn can_focus(&self) -> bool {
        true
    }

    fn has_focus(&self) -> bool {
        self.focused
    }

    fn set_focus(&mut self, focused: bool) {
        self.focused = focused;
        if focused && self.enabled {
            self.state = GadgetState::Hovered;
        } else if !focused {
            self.state = GadgetState::Normal;
        }
    }

    fn handle_input(&mut self, event: &InputEvent) -> Vec<GadgetMessage> {
        if !self.enabled || !self.visible {
            return Vec::new();
        }

        let mut messages = Vec::new();

        match event {
            InputEvent::MouseDown { .. } => {
                // Set focus when clicked
                if !self.focused {
                    messages.push(GadgetMessage::FocusChanged {
                        gadget_id: self.id,
                        has_focus: true,
                    });
                }
                self.state = GadgetState::Hovered;
            }
            InputEvent::MouseEnter { .. } => {
                if !self.focused {
                    self.state = GadgetState::Hovered;
                }
                messages.push(GadgetMessage::MouseEnter { gadget_id: self.id });
            }
            InputEvent::MouseLeave { .. } => {
                if !self.focused {
                    self.state = GadgetState::Normal;
                }
                messages.push(GadgetMessage::MouseLeave { gadget_id: self.id });
            }
            InputEvent::MouseDrag { button, .. } => {
                if *button == MouseButton::Left {
                    messages.push(GadgetMessage::LeftDrag { gadget_id: self.id });
                }
            }

            InputEvent::KeyDown { key, modifiers } => {
                if !self.focused {
                    return messages;
                }

                if modifiers.ctrl || modifiers.alt {
                    return messages;
                }

                let before = self.text.clone();

                match key {
                    KeyCode::Backspace => {
                        self.delete_before_cursor();
                    }

                    KeyCode::Enter => {
                        if self.is_multiline {
                            self.insert_char('\n');
                        } else {
                            // Submit the text entry
                            self.trigger_submit_callback();
                            messages.push(GadgetMessage::EditingComplete {
                                gadget_id: self.id,
                                text: self.text.clone(),
                            });
                        }
                    }

                    KeyCode::Tab | KeyCode::Right | KeyCode::Down => {
                        messages.push(GadgetMessage::Custom {
                            gadget_id: self.id,
                            data: "tab_next".to_string(),
                        });
                    }

                    KeyCode::Left | KeyCode::Up => {
                        messages.push(GadgetMessage::Custom {
                            gadget_id: self.id,
                            data: "tab_prev".to_string(),
                        });
                    }

                    KeyCode::Char(ch) => {
                        self.insert_char(*ch);
                    }

                    _ => {}
                }

                if self.text != before {
                    messages.push(GadgetMessage::ValueChanged {
                        gadget_id: self.id,
                        value: GadgetValue::String(self.text.clone()),
                    });
                }
            }

            InputEvent::TextInput { text } => {
                if self.focused {
                    if self.has_selection() {
                        self.delete_selection();
                    }
                    self.insert_text(text);
                }
            }
            InputEvent::FocusGained => {
                self.set_focus(true);
                messages.push(GadgetMessage::FocusChanged {
                    gadget_id: self.id,
                    has_focus: true,
                });
            }

            InputEvent::FocusLost => {
                self.set_focus(false);
                messages.push(GadgetMessage::FocusChanged {
                    gadget_id: self.id,
                    has_focus: false,
                });
            }

            _ => {}
        }

        messages
    }

    fn update(&mut self, delta_time: f32) {
        // Handle key repeat
        if let Some(key) = self.repeat_key {
            self.repeat_timer += delta_time;

            if self.repeat_timer >= self.repeat_delay {
                // Simulate repeated key press
                let event = InputEvent::KeyDown {
                    key,
                    modifiers: KeyModifiers::none(),
                };
                self.handle_input(&event);

                self.repeat_timer = 0.0;
                self.repeat_delay = self.repeat_rate; // Use faster rate for subsequent repeats
            }
        }
    }

    fn render(&self, theme: &GadgetTheme) {
        // Placeholder rendering code
        let text_color = self.get_text_color(theme);
        let bg_color = self.background_color.unwrap_or(Color::WHITE);
        let border_color = self.border_color.unwrap_or(theme.border_color);

        println!(
            "Rendering text entry {} at ({}, {}) {}x{}",
            self.id, self.bounds.x, self.bounds.y, self.bounds.width, self.bounds.height
        );

        println!("  Background: {:?}", bg_color);
        println!("  Border: {:?}", border_color);
        println!("  Text color: {:?}", text_color);

        if self.text.is_empty() && !self.placeholder.is_empty() {
            println!("  Placeholder: '{}'", self.placeholder);
        } else {
            println!("  Text: '{}'", self.displayed_text);
        }

        if self.focused {
            println!("  Cursor at position: {}", self.cursor_position);
            if self.has_selection() {
                println!(
                    "  Selection: {:?} to {:?}",
                    self.selection_start, self.selection_end
                );
            }
        }

        println!("  Validation: {:?}", self.validation_mode);
        if self.is_password {
            println!("  Password field");
        }
        if self.is_multiline {
            println!("  Multiline");
        }
    }

    fn handle_tab(&mut self, _direction: TabDirection) -> bool {
        true // Text entries participate in tab navigation
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_static_text_creation() {
        let text = StaticText::new(1, 10, 20, 200, 30)
            .with_text("Hello World")
            .with_alignment(TextAlignment::Center, VerticalAlignment::Center);

        assert_eq!(text.text(), "Hello World");
        assert_eq!(text.config.alignment, TextAlignment::Center);
        assert_eq!(text.config.vertical_alignment, VerticalAlignment::Center);
    }

    #[test]
    fn test_text_entry_basic() {
        let mut entry = TextEntry::new(1, 10, 20, 200, 30)
            .with_max_length(50)
            .with_placeholder("Enter text...");

        assert_eq!(entry.text(), "");
        assert_eq!(entry.placeholder(), "Enter text...");
        assert_eq!(entry.max_length, 50);

        entry.set_text("Hello");
        assert_eq!(entry.text(), "Hello");
        assert_eq!(entry.cursor_position, 5);
    }

    #[test]
    fn test_text_entry_validation() {
        let mut entry =
            TextEntry::new(1, 0, 0, 100, 30).with_validation(ValidationMode::NumericOnly);

        entry.set_text("abc123def456");
        assert_eq!(entry.text(), "123456");

        entry.set_validation(ValidationMode::AlphanumericOnly);
        entry.set_text("hello123!@#world456");
        assert_eq!(entry.text(), "hello123world456");
    }

    #[test]
    fn test_text_entry_password() {
        let mut entry = TextEntry::new(1, 0, 0, 100, 30).as_password();

        entry.set_text("password123");
        assert_eq!(entry.text(), "password123");
        assert_eq!(entry.displayed_text, "***********");
        assert!(entry.is_password());
    }

    #[test]
    fn test_cursor_movement() {
        let mut entry = TextEntry::new(1, 0, 0, 100, 30);
        entry.set_text("Hello World");

        // Move cursor to beginning
        entry.move_cursor_home(false);
        assert_eq!(entry.cursor_position, 0);
        assert!(!entry.has_selection());

        // Move cursor to end with selection
        entry.move_cursor_end(true);
        assert_eq!(entry.cursor_position, 11);
        assert!(entry.has_selection());
        assert_eq!(entry.selected_text(), Some("Hello World".to_string()));
    }

    #[test]
    fn test_text_editing() {
        let mut entry = TextEntry::new(1, 0, 0, 100, 30);
        entry.set_text("Hello World");
        entry.cursor_position = 6; // After "Hello "

        // Insert text
        entry.insert_text("Beautiful ");
        assert_eq!(entry.text(), "Hello Beautiful World");
        assert_eq!(entry.cursor_position, 16);

        // Delete before cursor
        entry.delete_before_cursor();
        assert_eq!(entry.text(), "Hello Beautifu World");
    }

    #[test]
    fn test_selection_operations() {
        let mut entry = TextEntry::new(1, 0, 0, 100, 30);
        entry.set_text("Hello World");

        entry.select_all();
        assert!(entry.has_selection());
        assert_eq!(entry.selected_text(), Some("Hello World".to_string()));

        let cut_text = entry.cut();
        assert_eq!(cut_text, Some("Hello World".to_string()));
        assert_eq!(entry.text(), "");

        entry.paste("New Text");
        assert_eq!(entry.text(), "New Text");
    }

    #[test]
    fn test_max_length_enforcement() {
        let mut entry = TextEntry::new(1, 0, 0, 100, 30).with_max_length(5);

        entry.set_text("12345678");
        assert_eq!(entry.text(), "12345");

        entry.insert_text("more");
        assert_eq!(entry.text(), "12345"); // Should not exceed max length
    }

    #[test]
    fn test_focus_event_emits_single_focus_changed_message() {
        let mut entry = TextEntry::new(1, 0, 0, 100, 30);

        let gained = entry.handle_input(&InputEvent::FocusGained);
        assert_eq!(gained.len(), 1);
        assert!(matches!(
            gained[0],
            GadgetMessage::FocusChanged {
                gadget_id: 1,
                has_focus: true
            }
        ));

        let lost = entry.handle_input(&InputEvent::FocusLost);
        assert_eq!(lost.len(), 1);
        assert!(matches!(
            lost[0],
            GadgetMessage::FocusChanged {
                gadget_id: 1,
                has_focus: false
            }
        ));
    }
}
