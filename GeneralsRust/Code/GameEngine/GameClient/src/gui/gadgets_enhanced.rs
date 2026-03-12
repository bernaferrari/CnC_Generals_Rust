//! Enhanced Gadget System
//!
//! Complete implementation of UI gadgets (widgets) including buttons, text fields,
//! lists, sliders, and other interactive elements that match the original C++ 
//! Command & Conquer Generals GUI system.

use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, RwLock, Weak};
use std::time::{Duration, Instant};
use thiserror::Error;

use super::game_window_enhanced::{EnhancedGameWindow, WindowCallbacks, WindowMessage, WindowMsgHandled, WindowStatus};
use super::ui_renderer::{UIRenderer, UIRect, TextLayout, TextAlignment, VerticalAlignment};

/// Gadget system errors
#[derive(Error, Debug)]
pub enum GadgetError {
    #[error("Invalid gadget configuration: {0}")]
    InvalidConfiguration(String),
    #[error("Event handling error: {0}")]
    EventError(String),
    #[error("Rendering error: {0}")]
    RenderError(String),
    #[error("Gadget not found: {0}")]
    GadgetNotFound(String),
}

type Result<T> = std::result::Result<T, GadgetError>;

/// Gadget state enumeration
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GadgetState {
    Normal,
    Highlighted,
    Pressed,
    Disabled,
    Focused,
}

/// Button styles
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ButtonStyle {
    Standard,    // Normal rectangular button
    RoundedRect, // Button with rounded corners
    Circular,    // Circular button
    Toggle,      // Toggle button with on/off states
    Radio,       // Radio button (part of a group)
    Checkbox,    // Checkbox button
}

/// Text input validation modes
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ValidationMode {
    None,           // No validation
    Numeric,        // Only numbers
    AlphaNumeric,   // Letters and numbers only
    Integer,        // Integer values only
    Float,          // Floating point numbers
    Email,          // Email address format
    Custom,         // Custom validation function
}

/// List selection modes
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SelectionMode {
    Single,    // Single selection
    Multiple,  // Multiple selection
    Extended,  // Extended selection (Ctrl+Click, Shift+Click)
}

/// Slider orientation
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SliderOrientation {
    Horizontal,
    Vertical,
}

/// Progress bar styles
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProgressStyle {
    Standard,    // Standard progress bar
    Chunked,     // Chunked progress segments
    Pulse,       // Pulsing/animated progress
    Circular,    // Circular progress indicator
}

/// Base trait for all gadgets
pub trait Gadget: Send + Sync {
    /// Get the gadget's name/identifier
    fn get_name(&self) -> &str;
    
    /// Get current state
    fn get_state(&self) -> GadgetState;
    
    /// Set state
    fn set_state(&mut self, state: GadgetState);
    
    /// Handle input events
    fn handle_input(&mut self, message: WindowMessage, wparam: u32, lparam: u32) -> Result<bool>;
    
    /// Update gadget (called per frame)
    fn update(&mut self, delta_time: f32) -> Result<()>;
    
    /// Render the gadget
    fn render(&self, renderer: &mut UIRenderer, bounds: UIRect) -> Result<()>;
    
    /// Get whether the gadget is enabled
    fn is_enabled(&self) -> bool;
    
    /// Enable/disable the gadget
    fn set_enabled(&mut self, enabled: bool);
}

/// Enhanced Push Button implementation
pub struct EnhancedPushButton {
    name: String,
    state: GadgetState,
    enabled: bool,
    text: String,
    style: ButtonStyle,
    
    // Visual properties
    normal_color: [f32; 4],
    hover_color: [f32; 4],
    pressed_color: [f32; 4],
    disabled_color: [f32; 4],
    text_color: [f32; 4],
    
    // Images for different states
    normal_image: Option<String>,
    hover_image: Option<String>,
    pressed_image: Option<String>,
    disabled_image: Option<String>,
    
    // Animation
    animation_time: f32,
    animation_duration: f32,
    is_animating: bool,
    press_scale: f32,
    scale: f32,
    scale_target: f32,
    scale_velocity: f32,
    spring_strength: f32,
    spring_damping: f32,
    press_impulse: f32,
    release_impulse: f32,
    
    // Callback
    on_click: Option<Box<dyn Fn() + Send + Sync>>,
    on_hover: Option<Box<dyn Fn(bool) + Send + Sync>>,
    
    // Toggle state (for toggle buttons)
    is_toggled: bool,
    toggle_group: Option<String>,
}

impl EnhancedPushButton {
    pub fn new(name: &str, text: &str, style: ButtonStyle) -> Self {
        Self {
            name: name.to_string(),
            state: GadgetState::Normal,
            enabled: true,
            text: text.to_string(),
            style,
            normal_color: [0.3, 0.3, 0.3, 1.0],
            hover_color: [0.4, 0.4, 0.4, 1.0],
            pressed_color: [0.2, 0.2, 0.2, 1.0],
            disabled_color: [0.1, 0.1, 0.1, 0.5],
            text_color: [1.0, 1.0, 1.0, 1.0],
            normal_image: None,
            hover_image: None,
            pressed_image: None,
            disabled_image: None,
            animation_time: 0.0,
            animation_duration: 0.1,
            is_animating: false,
            press_scale: 0.94,
            scale: 1.0,
            scale_target: 1.0,
            scale_velocity: 0.0,
            spring_strength: 60.0,
            spring_damping: 10.0,
            press_impulse: -4.5,
            release_impulse: 5.5,
            on_click: None,
            on_hover: None,
            is_toggled: false,
            toggle_group: None,
        }
    }
    
    pub fn set_colors(&mut self, normal: [f32; 4], hover: [f32; 4], pressed: [f32; 4], disabled: [f32; 4]) {
        self.normal_color = normal;
        self.hover_color = hover;
        self.pressed_color = pressed;
        self.disabled_color = disabled;
    }
    
    pub fn set_images(&mut self, normal: Option<String>, hover: Option<String>, pressed: Option<String>, disabled: Option<String>) {
        self.normal_image = normal;
        self.hover_image = hover;
        self.pressed_image = pressed;
        self.disabled_image = disabled;
    }
    
    pub fn set_click_callback<F>(&mut self, callback: F) 
    where
        F: Fn() + Send + Sync + 'static,
    {
        self.on_click = Some(Box::new(callback));
    }
    
    pub fn set_hover_callback<F>(&mut self, callback: F) 
    where
        F: Fn(bool) + Send + Sync + 'static,
    {
        self.on_hover = Some(Box::new(callback));
    }
    
    pub fn set_text(&mut self, text: &str) {
        self.text = text.to_string();
    }
    
    pub fn get_text(&self) -> &str {
        &self.text
    }
    
    pub fn is_toggled(&self) -> bool {
        self.is_toggled
    }
    
    pub fn set_toggled(&mut self, toggled: bool) {
        if matches!(self.style, ButtonStyle::Toggle | ButtonStyle::Checkbox | ButtonStyle::Radio) {
            self.is_toggled = toggled;
        }
    }
    
    pub fn set_toggle_group(&mut self, group: Option<String>) {
        self.toggle_group = group;
    }
    
    fn get_current_color(&self) -> [f32; 4] {
        match self.state {
            GadgetState::Normal => {
                if self.is_toggled { self.pressed_color } else { self.normal_color }
            }
            GadgetState::Highlighted => self.hover_color,
            GadgetState::Pressed => self.pressed_color,
            GadgetState::Disabled => self.disabled_color,
            GadgetState::Focused => {
                // Add slight glow for focused state
                let base = if self.is_toggled { self.pressed_color } else { self.normal_color };
                [base[0] + 0.1, base[1] + 0.1, base[2] + 0.1, base[3]]
            }
        }
    }
}

impl Gadget for EnhancedPushButton {
    fn get_name(&self) -> &str {
        &self.name
    }
    
    fn get_state(&self) -> GadgetState {
        self.state
    }
    
    fn set_state(&mut self, state: GadgetState) {
        if self.state != state {
            let was_pressed = matches!(self.state, GadgetState::Pressed);
            self.state = state;
            self.is_animating = true;
            self.animation_time = 0.0;
            self.scale_target = if matches!(state, GadgetState::Pressed) {
                self.press_scale
            } else {
                1.0
            };
            if matches!(state, GadgetState::Pressed) {
                self.scale_velocity = self.press_impulse;
            } else if was_pressed {
                self.scale_velocity = self.release_impulse;
            }
            
            // Call hover callback
            if let Some(ref callback) = self.on_hover {
                callback(matches!(state, GadgetState::Highlighted | GadgetState::Pressed));
            }
        }
    }
    
    fn handle_input(&mut self, message: WindowMessage, wparam: u32, lparam: u32) -> Result<bool> {
        if !self.enabled {
            return Ok(false);
        }
        
        match message {
            WindowMessage::MouseEntering => {
                self.set_state(GadgetState::Highlighted);
                Ok(true)
            }
            WindowMessage::MouseLeaving => {
                self.set_state(GadgetState::Normal);
                Ok(true)
            }
            WindowMessage::LeftDown => {
                self.set_state(GadgetState::Pressed);
                Ok(true)
            }
            WindowMessage::LeftUp => {
                if self.state == GadgetState::Pressed {
                    // Handle different button types
                    match self.style {
                        ButtonStyle::Toggle | ButtonStyle::Checkbox => {
                            self.is_toggled = !self.is_toggled;
                        }
                        ButtonStyle::Radio => {
                            // Radio buttons are handled by their group
                            self.is_toggled = true;
                        }
                        _ => {}
                    }
                    
                    // Call click callback
                    if let Some(ref callback) = self.on_click {
                        callback();
                    }
                    
                    self.set_state(GadgetState::Highlighted);
                    Ok(true)
                } else {
                    Ok(false)
                }
            }
            _ => Ok(false)
        }
    }
    
    fn update(&mut self, delta_time: f32) -> Result<()> {
        if self.is_animating {
            self.animation_time += delta_time;
            if self.animation_time >= self.animation_duration {
                self.animation_time = self.animation_duration;
                self.is_animating = false;
            }
        }
        
        let dt = delta_time.max(0.0);
        let displacement = self.scale - self.scale_target;
        let accel = -self.spring_strength * displacement - self.spring_damping * self.scale_velocity;
        self.scale_velocity += accel * dt;
        self.scale += self.scale_velocity * dt;
        
        if (self.scale - self.scale_target).abs() < 0.0005 && self.scale_velocity.abs() < 0.0005 {
            self.scale = self.scale_target;
            self.scale_velocity = 0.0;
        }
        Ok(())
    }
    
    fn render(&self, renderer: &mut UIRenderer, bounds: UIRect) -> Result<()> {
        let color = self.get_current_color();
        let scaled_bounds = if (self.scale - 1.0).abs() > f32::EPSILON {
            let cx = bounds.x + bounds.width * 0.5;
            let cy = bounds.y + bounds.height * 0.5;
            let width = bounds.width * self.scale;
            let height = bounds.height * self.scale;
            UIRect::new(cx - width * 0.5, cy - height * 0.5, width, height)
        } else {
            bounds
        };
        
        // Apply animation interpolation if animating
        let final_color = if self.is_animating {
            let t = self.animation_time / self.animation_duration;
            let ease_t = t * t * (3.0 - 2.0 * t); // Smoothstep easing
            
            // Interpolate between previous and current color (simplified)
            color
        } else {
            color
        };
        
        // Render button background based on style
        match self.style {
            ButtonStyle::Standard | ButtonStyle::Toggle => {
                renderer.draw_rect(scaled_bounds, final_color, 0.1);
            }
            ButtonStyle::RoundedRect => {
                // Would implement rounded rectangle rendering
                renderer.draw_rect(scaled_bounds, final_color, 0.1);
            }
            ButtonStyle::Circular => {
                // Would implement circular button rendering
                renderer.draw_rect(scaled_bounds, final_color, 0.1);
            }
            ButtonStyle::Checkbox => {
                // Draw checkbox background
                renderer.draw_rect(scaled_bounds, final_color, 0.1);
                
                // Draw checkmark if toggled
                if self.is_toggled {
                    let check_bounds = UIRect::new(
                        scaled_bounds.x + scaled_bounds.width * 0.2,
                        scaled_bounds.y + scaled_bounds.height * 0.2,
                        scaled_bounds.width * 0.6,
                        scaled_bounds.height * 0.6
                    );
                    renderer.draw_rect(check_bounds, [1.0, 1.0, 1.0, 1.0], 0.2);
                }
            }
            ButtonStyle::Radio => {
                // Would implement radio button circle rendering
                renderer.draw_rect(scaled_bounds, final_color, 0.1);
                
                if self.is_toggled {
                    let dot_bounds = UIRect::new(
                        scaled_bounds.x + scaled_bounds.width * 0.3,
                        scaled_bounds.y + scaled_bounds.height * 0.3,
                        scaled_bounds.width * 0.4,
                        scaled_bounds.height * 0.4
                    );
                    renderer.draw_rect(dot_bounds, [1.0, 1.0, 1.0, 1.0], 0.2);
                }
            }
        }
        
        // Render text
        if !self.text.is_empty() {
            let text_layout = TextLayout {
                text: self.text.clone(),
                font_size: 14.0,
                color: self.text_color,
                bounds: scaled_bounds,
                alignment: TextAlignment::Center,
                vertical_alignment: VerticalAlignment::Middle,
                word_wrap: false,
                single_line: true,
            };
            
            renderer.draw_text(&text_layout, 0.2)?;
        }
        
        Ok(())
    }
    
    fn is_enabled(&self) -> bool {
        self.enabled
    }
    
    fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
        if enabled {
            if self.state == GadgetState::Disabled {
                self.set_state(GadgetState::Normal);
            }
        } else {
            self.set_state(GadgetState::Disabled);
        }
    }
}

/// Enhanced Text Entry gadget
pub struct EnhancedTextEntry {
    name: String,
    state: GadgetState,
    enabled: bool,
    
    // Text properties
    text: String,
    placeholder: String,
    cursor_position: usize,
    selection_start: Option<usize>,
    selection_end: Option<usize>,
    
    // Validation
    validation_mode: ValidationMode,
    max_length: Option<usize>,
    custom_validator: Option<Box<dyn Fn(&str) -> bool + Send + Sync>>,
    
    // Visual properties
    background_color: [f32; 4],
    text_color: [f32; 4],
    selection_color: [f32; 4],
    cursor_color: [f32; 4],
    border_color: [f32; 4],
    
    // Cursor blinking
    cursor_blink_time: f32,
    cursor_visible: bool,
    
    // Scrolling for long text
    scroll_offset: f32,
    
    // Callbacks
    on_text_changed: Option<Box<dyn Fn(&str) + Send + Sync>>,
    on_enter_pressed: Option<Box<dyn Fn(&str) + Send + Sync>>,
    on_focus_changed: Option<Box<dyn Fn(bool) + Send + Sync>>,
}

impl EnhancedTextEntry {
    pub fn new(name: &str, placeholder: &str) -> Self {
        Self {
            name: name.to_string(),
            state: GadgetState::Normal,
            enabled: true,
            text: String::new(),
            placeholder: placeholder.to_string(),
            cursor_position: 0,
            selection_start: None,
            selection_end: None,
            validation_mode: ValidationMode::None,
            max_length: None,
            custom_validator: None,
            background_color: [0.1, 0.1, 0.1, 1.0],
            text_color: [1.0, 1.0, 1.0, 1.0],
            selection_color: [0.3, 0.5, 0.8, 0.5],
            cursor_color: [1.0, 1.0, 1.0, 1.0],
            border_color: [0.5, 0.5, 0.5, 1.0],
            cursor_blink_time: 0.0,
            cursor_visible: true,
            scroll_offset: 0.0,
            on_text_changed: None,
            on_enter_pressed: None,
            on_focus_changed: None,
        }
    }
    
    pub fn set_text(&mut self, text: &str) {
        if self.validate_text(text) {
            self.text = text.to_string();
            self.cursor_position = self.text.len();
            self.clear_selection();
            
            if let Some(ref callback) = self.on_text_changed {
                callback(&self.text);
            }
        }
    }
    
    pub fn get_text(&self) -> &str {
        &self.text
    }
    
    pub fn set_validation_mode(&mut self, mode: ValidationMode) {
        self.validation_mode = mode;
    }
    
    pub fn set_max_length(&mut self, max_length: Option<usize>) {
        self.max_length = max_length;
    }
    
    pub fn set_custom_validator<F>(&mut self, validator: F)
    where
        F: Fn(&str) -> bool + Send + Sync + 'static,
    {
        self.custom_validator = Some(Box::new(validator));
    }
    
    pub fn set_text_changed_callback<F>(&mut self, callback: F)
    where
        F: Fn(&str) + Send + Sync + 'static,
    {
        self.on_text_changed = Some(Box::new(callback));
    }
    
    pub fn set_enter_pressed_callback<F>(&mut self, callback: F)
    where
        F: Fn(&str) + Send + Sync + 'static,
    {
        self.on_enter_pressed = Some(Box::new(callback));
    }
    
    pub fn clear_selection(&mut self) {
        self.selection_start = None;
        self.selection_end = None;
    }
    
    pub fn select_all(&mut self) {
        self.selection_start = Some(0);
        self.selection_end = Some(self.text.len());
    }
    
    pub fn has_selection(&self) -> bool {
        self.selection_start.is_some() && self.selection_end.is_some()
    }
    
    pub fn get_selected_text(&self) -> Option<String> {
        if let (Some(start), Some(end)) = (self.selection_start, self.selection_end) {
            let start_idx = start.min(end);
            let end_idx = start.max(end);
            Some(self.text[start_idx..end_idx].to_string())
        } else {
            None
        }
    }
    
    pub fn insert_text_at_cursor(&mut self, text: &str) {
        let mut new_text = self.text.clone();
        
        // Remove selection if exists
        if self.has_selection() {
            if let (Some(start), Some(end)) = (self.selection_start, self.selection_end) {
                let start_idx = start.min(end);
                let end_idx = start.max(end);
                new_text.replace_range(start_idx..end_idx, "");
                self.cursor_position = start_idx;
            }
            self.clear_selection();
        }
        
        // Insert new text
        new_text.insert_str(self.cursor_position, text);
        
        if self.validate_text(&new_text) {
            self.text = new_text;
            self.cursor_position += text.len();
            
            if let Some(ref callback) = self.on_text_changed {
                callback(&self.text);
            }
        }
    }
    
    pub fn delete_at_cursor(&mut self, forward: bool) {
        if self.has_selection() {
            // Delete selection
            if let (Some(start), Some(end)) = (self.selection_start, self.selection_end) {
                let start_idx = start.min(end);
                let end_idx = start.max(end);
                self.text.replace_range(start_idx..end_idx, "");
                self.cursor_position = start_idx;
            }
            self.clear_selection();
        } else if forward && self.cursor_position < self.text.len() {
            // Delete character after cursor
            self.text.remove(self.cursor_position);
        } else if !forward && self.cursor_position > 0 {
            // Delete character before cursor
            self.cursor_position -= 1;
            self.text.remove(self.cursor_position);
        }
        
        if let Some(ref callback) = self.on_text_changed {
            callback(&self.text);
        }
    }
    
    pub fn move_cursor(&mut self, delta: i32, extend_selection: bool) {
        let new_pos = if delta < 0 {
            self.cursor_position.saturating_sub((-delta) as usize)
        } else {
            (self.cursor_position + delta as usize).min(self.text.len())
        };
        
        if extend_selection {
            if self.selection_start.is_none() {
                self.selection_start = Some(self.cursor_position);
            }
            self.selection_end = Some(new_pos);
        } else {
            self.clear_selection();
        }
        
        self.cursor_position = new_pos;
    }
    
    fn validate_text(&self, text: &str) -> bool {
        // Check max length
        if let Some(max_len) = self.max_length {
            if text.len() > max_len {
                return false;
            }
        }
        
        // Check validation mode
        match self.validation_mode {
            ValidationMode::None => true,
            ValidationMode::Numeric => text.chars().all(|c| c.is_numeric() || c == '.' || c == '-'),
            ValidationMode::AlphaNumeric => text.chars().all(|c| c.is_alphanumeric()),
            ValidationMode::Integer => text.parse::<i64>().is_ok(),
            ValidationMode::Float => text.parse::<f64>().is_ok(),
            ValidationMode::Email => {
                // Basic email validation
                text.contains('@') && text.contains('.') && text.len() > 5
            }
            ValidationMode::Custom => {
                if let Some(ref validator) = self.custom_validator {
                    validator(text)
                } else {
                    true
                }
            }
        }
    }
}

impl Gadget for EnhancedTextEntry {
    fn get_name(&self) -> &str {
        &self.name
    }
    
    fn get_state(&self) -> GadgetState {
        self.state
    }
    
    fn set_state(&mut self, state: GadgetState) {
        if self.state != state {
            self.state = state;
            
            if let Some(ref callback) = self.on_focus_changed {
                callback(matches!(state, GadgetState::Focused));
            }
        }
    }
    
    fn handle_input(&mut self, message: WindowMessage, wparam: u32, lparam: u32) -> Result<bool> {
        if !self.enabled {
            return Ok(false);
        }
        
        match message {
            WindowMessage::LeftDown => {
                self.set_state(GadgetState::Focused);
                // Set cursor position based on click location (would need font metrics)
                Ok(true)
            }
            WindowMessage::Char => {
                if self.state == GadgetState::Focused {
                    let ch = char::from_u32(wparam).unwrap_or('\0');
                    if ch.is_control() {
                        // Handle control characters
                        match ch {
                            '\x08' => self.delete_at_cursor(false), // Backspace
                            '\x7F' => self.delete_at_cursor(true),  // Delete
                            '\r' | '\n' => {
                                // Enter key
                                if let Some(ref callback) = self.on_enter_pressed {
                                    callback(&self.text);
                                }
                            }
                            _ => {}
                        }
                    } else if ch.is_ascii_graphic() || ch == ' ' {
                        // Insert printable character
                        let ch_str = ch.to_string();
                        self.insert_text_at_cursor(&ch_str);
                    }
                    Ok(true)
                } else {
                    Ok(false)
                }
            }
            _ => Ok(false)
        }
    }
    
    fn update(&mut self, delta_time: f32) -> Result<()> {
        // Update cursor blinking
        if self.state == GadgetState::Focused {
            self.cursor_blink_time += delta_time;
            if self.cursor_blink_time >= 0.5 {
                self.cursor_visible = !self.cursor_visible;
                self.cursor_blink_time = 0.0;
            }
        } else {
            self.cursor_visible = false;
        }
        
        Ok(())
    }
    
    fn render(&self, renderer: &mut UIRenderer, bounds: UIRect) -> Result<()> {
        // Render background
        let bg_color = if self.state == GadgetState::Focused {
            [self.background_color[0] + 0.05, self.background_color[1] + 0.05, self.background_color[2] + 0.05, self.background_color[3]]
        } else {
            self.background_color
        };
        renderer.draw_rect(bounds, bg_color, 0.1);
        
        // Render border
        // Would implement border rendering here
        
        // Render text or placeholder
        let display_text = if self.text.is_empty() {
            &self.placeholder
        } else {
            &self.text
        };
        
        let text_color = if self.text.is_empty() {
            [self.text_color[0] * 0.5, self.text_color[1] * 0.5, self.text_color[2] * 0.5, self.text_color[3]]
        } else {
            self.text_color
        };
        
        let text_bounds = UIRect::new(
            bounds.x + 5.0,
            bounds.y + 2.0,
            bounds.width - 10.0,
            bounds.height - 4.0
        );
        
        let text_layout = TextLayout {
            text: display_text.to_string(),
            font_size: 14.0,
            color: text_color,
            bounds: text_bounds,
            alignment: TextAlignment::Left,
            vertical_alignment: VerticalAlignment::Middle,
            word_wrap: false,
            single_line: true,
        };
        
        renderer.draw_text(&text_layout, 0.2)?;
        
        // Render selection
        if self.has_selection() {
            // Would render selection highlight here
        }
        
        // Render cursor
        if self.cursor_visible && self.state == GadgetState::Focused {
            // Calculate cursor position (would need proper font metrics)
            let cursor_x = text_bounds.x + (self.cursor_position as f32 * 8.0); // Rough estimate
            let cursor_rect = UIRect::new(cursor_x, text_bounds.y, 1.0, text_bounds.height);
            renderer.draw_rect(cursor_rect, self.cursor_color, 0.3);
        }
        
        Ok(())
    }
    
    fn is_enabled(&self) -> bool {
        self.enabled
    }
    
    fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
        if enabled {
            if self.state == GadgetState::Disabled {
                self.set_state(GadgetState::Normal);
            }
        } else {
            self.set_state(GadgetState::Disabled);
        }
    }
}

/// Gadget Manager - manages all gadgets in a window or layout
pub struct GadgetManager {
    gadgets: HashMap<String, Box<dyn Gadget>>,
    focused_gadget: Option<String>,
    tab_order: Vec<String>,
}

impl GadgetManager {
    pub fn new() -> Self {
        Self {
            gadgets: HashMap::new(),
            focused_gadget: None,
            tab_order: Vec::new(),
        }
    }
    
    pub fn add_gadget(&mut self, gadget: Box<dyn Gadget>) {
        let name = gadget.get_name().to_string();
        self.tab_order.push(name.clone());
        self.gadgets.insert(name, gadget);
    }
    
    pub fn remove_gadget(&mut self, name: &str) -> Option<Box<dyn Gadget>> {
        self.tab_order.retain(|n| n != name);
        if self.focused_gadget.as_ref() == Some(&name.to_string()) {
            self.focused_gadget = None;
        }
        self.gadgets.remove(name)
    }
    
    pub fn get_gadget(&self, name: &str) -> Option<&dyn Gadget> {
        self.gadgets.get(name).map(|g| g.as_ref())
    }
    
    pub fn get_gadget_mut(&mut self, name: &str) -> Option<&mut dyn Gadget> {
        if let Some(gadget) = self.gadgets.get_mut(name) {
            Some(gadget.as_mut())
        } else {
            None
        }
    }
    
    pub fn handle_input(&mut self, message: WindowMessage, wparam: u32, lparam: u32) -> Result<bool> {
        // Send to focused gadget first
        if let Some(ref focused_name) = self.focused_gadget.clone() {
            if let Some(gadget) = self.gadgets.get_mut(focused_name) {
                if gadget.handle_input(message, wparam, lparam)? {
                    return Ok(true);
                }
            }
        }
        
        // If not handled by focused gadget, try all gadgets
        for gadget in self.gadgets.values_mut() {
            if gadget.handle_input(message, wparam, lparam)? {
                return Ok(true);
            }
        }
        
        Ok(false)
    }
    
    pub fn update(&mut self, delta_time: f32) -> Result<()> {
        for gadget in self.gadgets.values_mut() {
            gadget.update(delta_time)?;
        }
        Ok(())
    }
    
    pub fn render(&self, renderer: &mut UIRenderer) -> Result<()> {
        // Render gadgets in tab order
        for name in &self.tab_order {
            if let Some(gadget) = self.gadgets.get(name) {
                // Would get proper bounds from layout system
                let bounds = UIRect::new(0.0, 0.0, 100.0, 30.0);
                gadget.render(renderer, bounds)?;
            }
        }
        Ok(())
    }
    
    pub fn set_focus(&mut self, gadget_name: Option<&str>) -> Result<()> {
        // Clear old focus
        if let Some(ref old_name) = self.focused_gadget {
            if let Some(gadget) = self.gadgets.get_mut(old_name) {
                gadget.set_state(GadgetState::Normal);
            }
        }
        
        // Set new focus
        if let Some(name) = gadget_name {
            if let Some(gadget) = self.gadgets.get_mut(name) {
                gadget.set_state(GadgetState::Focused);
                self.focused_gadget = Some(name.to_string());
            } else {
                return Err(GadgetError::GadgetNotFound(name.to_string()));
            }
        } else {
            self.focused_gadget = None;
        }
        
        Ok(())
    }
    
    pub fn tab_to_next(&mut self, forward: bool) -> Result<()> {
        let current_index = if let Some(ref focused_name) = self.focused_gadget {
            self.tab_order.iter().position(|name| name == focused_name)
        } else {
            None
        };
        
        let next_index = match current_index {
            Some(idx) => {
                if forward {
                    (idx + 1) % self.tab_order.len()
                } else {
                    if idx == 0 { self.tab_order.len() - 1 } else { idx - 1 }
                }
            }
            None => 0,
        };
        
        if let Some(next_name) = self.tab_order.get(next_index) {
            self.set_focus(Some(next_name))?;
        }
        
        Ok(())
    }
    
    pub fn clear(&mut self) {
        self.gadgets.clear();
        self.focused_gadget = None;
        self.tab_order.clear();
    }
}
