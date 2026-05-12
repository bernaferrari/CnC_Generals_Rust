//! Slider Controls Implementation
//!
//! This module provides slider controls for numeric value selection:
//! - **HorizontalSlider**: Left-to-right value selection
//! - **VerticalSlider**: Top-to-bottom value selection
//! - **Range sliders**: Two-thumb sliders for range selection
//!
//! # Features
//!
//! - Configurable value ranges with custom min/max values
//! - Continuous or discrete (stepped) value selection
//! - Visual thumb positioning with smooth dragging
//! - Keyboard navigation and precise value adjustment
//! - Custom styling for track and thumb elements
//! - Page-click navigation (click on track to jump)
//! - Real-time value change notifications
//! - Accessibility support with proper focus handling
//!
//! # Examples
//!
//! ## Basic Sliders
//!
//! ```rust
//! use game_client::gui::gadgets::slider::*;
//!
//! // Create a volume slider (0-100)
//! let volume_slider = HorizontalSlider::new(1, 20, 100, 200, 20)
//!     .with_range(0, 100)
//!     .with_value(75)
//!     .with_step_size(5) // Snap to multiples of 5
//!     .with_change_callback(Box::new(|id, value| {
//!         println!("Volume changed to: {}", value);
//!     }));
//!
//! // Create a vertical brightness slider
//! let brightness = VerticalSlider::new(2, 250, 50, 20, 150)
//!     .with_range(0, 255)
//!     .with_value(128)
//!     .with_smooth_scrolling(true);
//! ```
//!
//! ## Range Sliders
//!
//! ```rust
//! // Create a price range slider
//! let price_range = RangeSlider::new(3, 20, 200, 200, 20)
//!     .with_range(0, 1000)
//!     .with_values(100, 500) // Min: $100, Max: $500
//!     .with_change_callback(Box::new(|id, min, max| {
//!         println!("Price range: ${} - ${}", min, max);
//!     }));
//! ```

use super::*;
use std::cmp::{max, min};

/// Callback function for slider value changes
pub type SliderCallback = Box<dyn Fn(GadgetId, i32) + Send + Sync>;

/// Callback function for range slider changes  
pub type RangeSliderCallback = Box<dyn Fn(GadgetId, i32, i32) + Send + Sync>;

/// Orientation of the slider
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SliderOrientation {
    Horizontal,
    Vertical,
}

/// Draw command emitted by sliders for the UI renderer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SliderRenderCommand {
    Track {
        rect: Rect,
        color: Color,
        border_color: Color,
        border_width: u32,
    },
    Fill {
        rect: Rect,
        color: Color,
    },
    Thumb {
        rect: Rect,
        color: Color,
        border_color: Color,
        border_width: u32,
    },
    FocusOutline {
        rect: Rect,
        color: Color,
    },
    StepTicks {
        step_size: i32,
    },
}

/// Configuration for slider appearance and behavior
#[derive(Debug, Clone)]
pub struct SliderConfig {
    pub min_value: i32,
    pub max_value: i32,
    pub step_size: Option<i32>, // None for continuous, Some(n) for discrete steps
    pub page_size: i32,         // Amount to change when clicking on track
    pub show_track_fill: bool,  // Show filled portion of track
    pub thumb_size: (u32, u32), // Width and height of thumb
    pub smooth_scrolling: bool,
}

impl Default for SliderConfig {
    fn default() -> Self {
        Self {
            min_value: 0,
            max_value: 100,
            step_size: None,
            page_size: 10,
            show_track_fill: true,
            thumb_size: (16, 20),
            smooth_scrolling: true,
        }
    }
}

/// Visual styling for sliders
#[derive(Debug, Clone)]
pub struct SliderStyle {
    // Track colors
    pub track_color: Color,
    pub track_fill_color: Color,
    pub track_border_color: Color,

    // Thumb colors for different states
    pub thumb_normal_color: Color,
    pub thumb_hovered_color: Color,
    pub thumb_pressed_color: Color,
    pub thumb_disabled_color: Color,
    pub thumb_border_color: Color,

    // Dimensions
    pub track_thickness: u32,
    pub track_border_width: u32,
    pub thumb_border_width: u32,
}

impl Default for SliderStyle {
    fn default() -> Self {
        Self {
            track_color: Color::rgb(180, 180, 180),
            track_fill_color: Color::rgb(0, 120, 215),
            track_border_color: Color::rgb(96, 96, 96),

            thumb_normal_color: Color::rgb(240, 240, 240),
            thumb_hovered_color: Color::rgb(250, 250, 250),
            thumb_pressed_color: Color::rgb(200, 200, 200),
            thumb_disabled_color: Color::rgb(160, 160, 160),
            thumb_border_color: Color::rgb(64, 64, 64),

            track_thickness: 4,
            track_border_width: 1,
            thumb_border_width: 1,
        }
    }
}

/// State of the slider thumb for interaction
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ThumbState {
    Normal,
    Hovered,
    Pressed,
}

// ============================================================================
// Base Slider Implementation
// ============================================================================

/// Base slider functionality shared by horizontal and vertical sliders
struct SliderBase {
    // Base gadget properties
    id: GadgetId,
    bounds: Rect,
    state: GadgetState,
    enabled: bool,
    visible: bool,
    focused: bool,

    // Slider configuration
    orientation: SliderOrientation,
    config: SliderConfig,
    style: SliderStyle,

    // Current state
    current_value: i32,
    thumb_state: ThumbState,
    thumb_position: i32, // Pixel position along track

    // Interaction state
    mouse_inside: bool,
    thumb_dragging: bool,
    drag_offset: i32, // Offset from thumb center to mouse

    // Callbacks
    change_callback: Option<SliderCallback>,

    // Animation/smoothing
    target_value: i32,
    animation_speed: f32,
}

impl SliderBase {
    fn new(id: GadgetId, bounds: Rect, orientation: SliderOrientation) -> Self {
        let mut slider = Self {
            id,
            bounds,
            state: GadgetState::Normal,
            enabled: true,
            visible: true,
            focused: false,

            orientation,
            config: SliderConfig::default(),
            style: SliderStyle::default(),

            current_value: 0,
            thumb_state: ThumbState::Normal,
            thumb_position: 0,

            mouse_inside: false,
            thumb_dragging: false,
            drag_offset: 0,

            change_callback: None,

            target_value: 0,
            animation_speed: 10.0,
        };

        slider.update_thumb_position();
        slider
    }

    /// Set the value range
    fn set_range(&mut self, min_value: i32, max_value: i32) {
        self.config.min_value = min_value;
        self.config.max_value = max_value.max(min_value);

        // Clamp current value to new range
        self.current_value = self
            .current_value
            .max(self.config.min_value)
            .min(self.config.max_value);
        self.target_value = self.current_value;
        self.update_thumb_position();
    }

    /// Set the current value
    fn set_value(&mut self, value: i32) {
        let new_value = self.clamp_value(value);
        if new_value != self.current_value {
            self.current_value = new_value;
            self.target_value = new_value;
            self.update_thumb_position();
            self.trigger_change_callback();
        }
    }

    /// Get the current value
    fn value(&self) -> i32 {
        self.current_value
    }

    /// Set step size for discrete values
    fn set_step_size(&mut self, step_size: Option<i32>) {
        self.config.step_size = step_size;
        if let Some(_step) = step_size {
            // Re-snap current value to step
            self.set_value(self.current_value);
        }
    }

    /// Clamp value to valid range and apply stepping
    fn clamp_value(&self, value: i32) -> i32 {
        let clamped = value.max(self.config.min_value).min(self.config.max_value);

        if let Some(step) = self.config.step_size {
            if step > 0 {
                let steps_from_min = (clamped - self.config.min_value) / step;
                return self.config.min_value + (steps_from_min * step);
            }
        }

        clamped
    }

    /// Update thumb position based on current value
    fn update_thumb_position(&mut self) {
        let track_length = self.get_track_length();
        let value_range = self.config.max_value - self.config.min_value;

        if value_range > 0 {
            let value_ratio =
                (self.current_value - self.config.min_value) as f32 / value_range as f32;
            self.thumb_position = (value_ratio * track_length as f32) as i32;
        } else {
            self.thumb_position = 0;
        }
    }

    /// Get the length of the track in pixels
    fn get_track_length(&self) -> i32 {
        match self.orientation {
            SliderOrientation::Horizontal => {
                (self.bounds.width as i32) - (self.config.thumb_size.0 as i32)
            }
            SliderOrientation::Vertical => {
                (self.bounds.height as i32) - (self.config.thumb_size.1 as i32)
            }
        }
    }

    /// Convert pixel position to value
    fn pixel_to_value(&self, pixel_pos: i32) -> i32 {
        let track_length = self.get_track_length();
        if track_length <= 0 {
            return self.config.min_value;
        }

        let clamped_pos = pixel_pos.max(0).min(track_length);
        let ratio = clamped_pos as f32 / track_length as f32;
        let value_range = self.config.max_value - self.config.min_value;
        let raw_value = self.config.min_value + (ratio * value_range as f32) as i32;

        self.clamp_value(raw_value)
    }

    /// Convert value to pixel position
    fn value_to_pixel(&self, value: i32) -> i32 {
        let track_length = self.get_track_length();
        let value_range = self.config.max_value - self.config.min_value;

        if value_range > 0 {
            let ratio = (value - self.config.min_value) as f32 / value_range as f32;
            (ratio * track_length as f32) as i32
        } else {
            0
        }
    }

    /// Get thumb bounds for collision detection
    fn get_thumb_bounds(&self) -> Rect {
        let thumb_size = self.config.thumb_size;

        match self.orientation {
            SliderOrientation::Horizontal => Rect::new(
                self.bounds.x + self.thumb_position,
                self.bounds.y + (self.bounds.height as i32 - thumb_size.1 as i32) / 2,
                thumb_size.0,
                thumb_size.1,
            ),
            SliderOrientation::Vertical => Rect::new(
                self.bounds.x + (self.bounds.width as i32 - thumb_size.0 as i32) / 2,
                self.bounds.y + self.thumb_position,
                thumb_size.0,
                thumb_size.1,
            ),
        }
    }

    /// Check if point is over thumb
    fn is_point_over_thumb(&self, x: i32, y: i32) -> bool {
        self.get_thumb_bounds().contains_point(x, y)
    }

    /// Handle mouse press on slider
    fn handle_mouse_press(&mut self, x: i32, y: i32, button: MouseButton) -> Vec<GadgetMessage> {
        if !self.enabled {
            return Vec::new();
        }

        let mut messages = Vec::new();

        match button {
            MouseButton::Left => {
                if self.is_point_over_thumb(x, y) {
                    // Start dragging thumb
                    self.thumb_dragging = true;
                    self.thumb_state = ThumbState::Pressed;

                    // Calculate drag offset
                    let thumb_center = match self.orientation {
                        SliderOrientation::Horizontal => {
                            self.bounds.x
                                + self.thumb_position
                                + (self.config.thumb_size.0 as i32 / 2)
                        }
                        SliderOrientation::Vertical => {
                            self.bounds.y
                                + self.thumb_position
                                + (self.config.thumb_size.1 as i32 / 2)
                        }
                    };

                    self.drag_offset = match self.orientation {
                        SliderOrientation::Horizontal => x - thumb_center,
                        SliderOrientation::Vertical => y - thumb_center,
                    };
                } else {
                    // Click on track - jump to position
                    let track_pos = match self.orientation {
                        SliderOrientation::Horizontal => {
                            x - self.bounds.x - (self.config.thumb_size.0 as i32 / 2)
                        }
                        SliderOrientation::Vertical => {
                            y - self.bounds.y - (self.config.thumb_size.1 as i32 / 2)
                        }
                    };

                    let new_value = self.pixel_to_value(track_pos);

                    // Apply page-click logic (move by page_size towards target)
                    let current_pixel = self.value_to_pixel(self.current_value);
                    let target_pixel = self.value_to_pixel(new_value);

                    let page_pixels =
                        self.value_to_pixel(self.config.page_size) - self.value_to_pixel(0);
                    let move_pixels = if target_pixel > current_pixel {
                        page_pixels.min(target_pixel - current_pixel)
                    } else {
                        (-page_pixels).max(target_pixel - current_pixel)
                    };

                    let page_value = self.pixel_to_value(current_pixel + move_pixels);
                    self.set_value(page_value);
                }
            }
            _ => {}
        }

        messages
    }

    /// Handle mouse release
    fn handle_mouse_release(
        &mut self,
        _x: i32,
        _y: i32,
        button: MouseButton,
    ) -> Vec<GadgetMessage> {
        if !self.enabled {
            return Vec::new();
        }

        match button {
            MouseButton::Left => {
                if self.thumb_dragging {
                    self.thumb_dragging = false;
                    self.thumb_state = if self.mouse_inside {
                        ThumbState::Hovered
                    } else {
                        ThumbState::Normal
                    };
                    self.drag_offset = 0;
                }
            }
            _ => {}
        }

        Vec::new()
    }

    /// Handle mouse move/drag
    fn handle_mouse_move(&mut self, x: i32, y: i32) -> Vec<GadgetMessage> {
        if !self.enabled {
            return Vec::new();
        }

        if self.thumb_dragging {
            // Update value based on drag position
            let drag_pos = match self.orientation {
                SliderOrientation::Horizontal => {
                    x - self.bounds.x - (self.config.thumb_size.0 as i32 / 2) - self.drag_offset
                }
                SliderOrientation::Vertical => {
                    y - self.bounds.y - (self.config.thumb_size.1 as i32 / 2) - self.drag_offset
                }
            };

            let new_value = self.pixel_to_value(drag_pos);
            self.set_value(new_value);
        } else {
            // Update thumb hover state
            let was_hovered = self.thumb_state == ThumbState::Hovered;
            let is_hovered = self.is_point_over_thumb(x, y);

            if is_hovered && !was_hovered {
                self.thumb_state = ThumbState::Hovered;
            } else if !is_hovered && was_hovered {
                self.thumb_state = ThumbState::Normal;
            }
        }

        Vec::new()
    }

    /// Handle mouse enter
    fn handle_mouse_enter(&mut self) -> Vec<GadgetMessage> {
        self.mouse_inside = true;
        vec![GadgetMessage::MouseEnter { gadget_id: self.id }]
    }

    /// Handle mouse leave  
    fn handle_mouse_leave(&mut self) -> Vec<GadgetMessage> {
        self.mouse_inside = false;
        if !self.thumb_dragging {
            self.thumb_state = ThumbState::Normal;
        }
        vec![GadgetMessage::MouseLeave { gadget_id: self.id }]
    }

    /// Handle keyboard input
    fn handle_key_press(&mut self, key: KeyCode, _modifiers: KeyModifiers) -> Vec<GadgetMessage> {
        if !self.enabled || !self.focused {
            return Vec::new();
        }

        let step = self.config.step_size.unwrap_or(1).max(1);
        let large_step = self.config.page_size;

        let new_value = match (self.orientation, key) {
            (SliderOrientation::Horizontal, KeyCode::Right) => self.current_value - (step * 2),
            (SliderOrientation::Horizontal, KeyCode::Left) => self.current_value + (step * 2),
            (SliderOrientation::Horizontal, KeyCode::Up | KeyCode::Down | KeyCode::Tab) => {
                return Vec::new();
            }
            (SliderOrientation::Vertical, KeyCode::Up) => self.current_value + (step * 2),
            (SliderOrientation::Vertical, KeyCode::Down) => self.current_value - (step * 2),
            (SliderOrientation::Vertical, KeyCode::Left | KeyCode::Right | KeyCode::Tab) => {
                return Vec::new();
            }
            (_, KeyCode::PageUp) => match self.orientation {
                SliderOrientation::Horizontal => self.current_value + large_step,
                SliderOrientation::Vertical => self.current_value - large_step,
            },
            (_, KeyCode::PageDown) => match self.orientation {
                SliderOrientation::Horizontal => self.current_value - large_step,
                SliderOrientation::Vertical => self.current_value + large_step,
            },
            (_, KeyCode::Home) => self.config.min_value,
            (_, KeyCode::End) => self.config.max_value,
            _ => return Vec::new(),
        };

        let old_value = self.current_value;
        self.set_value(new_value);
        if self.current_value == old_value {
            vec![GadgetMessage::Custom {
                gadget_id: self.id,
                data: "key_handled".to_string(),
            }]
        } else {
            Vec::new()
        }
    }

    /// Update animation and smooth scrolling
    fn update_animation(&mut self, delta_time: f32) {
        if self.config.smooth_scrolling && self.current_value != self.target_value {
            let diff = self.target_value - self.current_value;
            let change = (diff as f32 * self.animation_speed * delta_time) as i32;

            if change.abs() >= diff.abs() || change == 0 {
                self.current_value = self.target_value;
            } else {
                self.current_value += change;
            }

            self.update_thumb_position();
        }
    }

    /// Trigger change callback
    fn trigger_change_callback(&self) {
        if let Some(ref callback) = self.change_callback {
            callback(self.id, self.current_value);
        }
    }

    /// Get current thumb color based on state
    fn get_thumb_color(&self) -> Color {
        if !self.enabled {
            return self.style.thumb_disabled_color;
        }

        match self.thumb_state {
            ThumbState::Normal => self.style.thumb_normal_color,
            ThumbState::Hovered => self.style.thumb_hovered_color,
            ThumbState::Pressed => self.style.thumb_pressed_color,
        }
    }

    fn get_track_rect(&self) -> Rect {
        match self.orientation {
            SliderOrientation::Horizontal => Rect::new(
                self.bounds.x,
                self.bounds.y + (self.bounds.height as i32 - self.style.track_thickness as i32) / 2,
                self.bounds.width,
                self.style.track_thickness,
            ),
            SliderOrientation::Vertical => Rect::new(
                self.bounds.x + (self.bounds.width as i32 - self.style.track_thickness as i32) / 2,
                self.bounds.y,
                self.style.track_thickness,
                self.bounds.height,
            ),
        }
    }

    fn get_fill_rect(&self) -> Option<Rect> {
        if !self.config.show_track_fill {
            return None;
        }

        let track_rect = self.get_track_rect();
        let thumb_bounds = self.get_thumb_bounds();
        match self.orientation {
            SliderOrientation::Horizontal => {
                let fill_width =
                    (thumb_bounds.x - track_rect.x + thumb_bounds.width as i32 / 2).max(0) as u32;
                Some(Rect::new(
                    track_rect.x,
                    track_rect.y,
                    fill_width.min(track_rect.width),
                    track_rect.height,
                ))
            }
            SliderOrientation::Vertical => {
                let fill_height =
                    (thumb_bounds.y - track_rect.y + thumb_bounds.height as i32 / 2).max(0) as u32;
                Some(Rect::new(
                    track_rect.x,
                    track_rect.y,
                    track_rect.width,
                    fill_height.min(track_rect.height),
                ))
            }
        }
    }

    fn render_commands(&self, theme: &GadgetTheme) -> Vec<SliderRenderCommand> {
        if !self.visible {
            return Vec::new();
        }

        let mut commands = vec![SliderRenderCommand::Track {
            rect: self.get_track_rect(),
            color: self.style.track_color,
            border_color: self.style.track_border_color,
            border_width: self.style.track_border_width,
        }];

        if let Some(fill_rect) = self.get_fill_rect() {
            commands.push(SliderRenderCommand::Fill {
                rect: fill_rect,
                color: self.style.track_fill_color,
            });
        }

        commands.push(SliderRenderCommand::Thumb {
            rect: self.get_thumb_bounds(),
            color: self.get_thumb_color(),
            border_color: self.style.thumb_border_color,
            border_width: self.style.thumb_border_width,
        });

        if self.focused {
            commands.push(SliderRenderCommand::FocusOutline {
                rect: self.bounds,
                color: theme.focused_color,
            });
        }

        if let Some(step_size) = self.config.step_size {
            commands.push(SliderRenderCommand::StepTicks { step_size });
        }

        commands
    }
}

// ============================================================================
// Horizontal Slider Implementation
// ============================================================================

/// Horizontal slider control for left-to-right value selection
pub struct HorizontalSlider {
    base: SliderBase,
}

impl HorizontalSlider {
    /// Create a new horizontal slider
    pub fn new(id: GadgetId, x: i32, y: i32, width: u32, height: u32) -> Self {
        Self {
            base: SliderBase::new(
                id,
                Rect::new(x, y, width, height),
                SliderOrientation::Horizontal,
            ),
        }
    }

    /// Set the value range
    pub fn with_range(mut self, min_value: i32, max_value: i32) -> Self {
        self.base.set_range(min_value, max_value);
        self
    }

    /// Set the current value
    pub fn with_value(mut self, value: i32) -> Self {
        self.base.set_value(value);
        self
    }

    /// Set step size for discrete values
    pub fn with_step_size(mut self, step_size: i32) -> Self {
        self.base.set_step_size(Some(step_size));
        self
    }

    /// Enable smooth scrolling animation
    pub fn with_smooth_scrolling(mut self, enabled: bool) -> Self {
        self.base.config.smooth_scrolling = enabled;
        self
    }

    /// Set change callback
    pub fn with_change_callback(mut self, callback: SliderCallback) -> Self {
        self.base.change_callback = Some(callback);
        self
    }

    pub fn set_change_callback<F>(&mut self, callback: F)
    where
        F: Fn(GadgetId, i32) + Send + Sync + 'static,
    {
        self.base.change_callback = Some(Box::new(callback));
    }

    /// Set page size for page-click navigation
    pub fn with_page_size(mut self, page_size: i32) -> Self {
        self.base.config.page_size = page_size;
        self
    }

    /// Set custom thumb size
    pub fn with_thumb_size(mut self, width: u32, height: u32) -> Self {
        self.base.config.thumb_size = (width, height);
        self.base.update_thumb_position();
        self
    }

    /// Set custom styling
    pub fn with_style(mut self, style: SliderStyle) -> Self {
        self.base.style = style;
        self
    }

    /// Get the current value
    pub fn value(&self) -> i32 {
        self.base.value()
    }

    /// Set the current value
    pub fn set_value(&mut self, value: i32) {
        self.base.set_value(value);
    }

    /// Get the value range
    pub fn range(&self) -> (i32, i32) {
        (self.base.config.min_value, self.base.config.max_value)
    }

    /// Set the value range
    pub fn set_range(&mut self, min_value: i32, max_value: i32) {
        self.base.set_range(min_value, max_value);
    }

    /// Build renderer-facing commands for the current slider state.
    pub fn render_commands(&self, theme: &GadgetTheme) -> Vec<SliderRenderCommand> {
        self.base.render_commands(theme)
    }
}

impl Gadget for HorizontalSlider {
    fn id(&self) -> GadgetId {
        self.base.id
    }

    fn bounds(&self) -> Rect {
        self.base.bounds
    }

    fn set_position(&mut self, x: i32, y: i32) {
        self.base.bounds.x = x;
        self.base.bounds.y = y;
    }

    fn set_size(&mut self, width: u32, height: u32) {
        self.base.bounds.width = width;
        self.base.bounds.height = height;
        self.base.update_thumb_position();
    }

    fn state(&self) -> GadgetState {
        self.base.state
    }

    fn is_enabled(&self) -> bool {
        self.base.enabled
    }

    fn set_enabled(&mut self, enabled: bool) {
        self.base.enabled = enabled;
        if !enabled {
            self.base.state = GadgetState::Disabled;
            self.base.thumb_dragging = false;
            self.base.thumb_state = ThumbState::Normal;
        }
    }

    fn is_visible(&self) -> bool {
        self.base.visible
    }

    fn set_visible(&mut self, visible: bool) {
        self.base.visible = visible;
    }

    fn can_focus(&self) -> bool {
        true
    }

    fn has_focus(&self) -> bool {
        self.base.focused
    }

    fn set_focus(&mut self, focused: bool) {
        self.base.focused = focused;
        if focused && self.base.enabled {
            self.base.state = GadgetState::Focused;
        } else if !focused {
            self.base.state = GadgetState::Normal;
        }
    }

    fn handle_input(&mut self, event: &InputEvent) -> Vec<GadgetMessage> {
        if !self.base.enabled || !self.base.visible {
            return Vec::new();
        }

        let prev_value = self.base.current_value;
        let mut messages = match event {
            InputEvent::MouseDown { x, y, button } => self.base.handle_mouse_press(*x, *y, *button),

            InputEvent::MouseUp { x, y, button } => self.base.handle_mouse_release(*x, *y, *button),

            InputEvent::MouseMove { x, y } | InputEvent::MouseDrag { x, y, .. } => {
                self.base.handle_mouse_move(*x, *y)
            }

            InputEvent::MouseEnter { .. } => self.base.handle_mouse_enter(),

            InputEvent::MouseLeave { .. } => self.base.handle_mouse_leave(),

            InputEvent::KeyDown { key, modifiers } => self.base.handle_key_press(*key, *modifiers),

            InputEvent::FocusGained => {
                self.set_focus(true);
                vec![GadgetMessage::FocusChanged {
                    gadget_id: self.base.id,
                    has_focus: true,
                }]
            }

            InputEvent::FocusLost => {
                self.set_focus(false);
                vec![GadgetMessage::FocusChanged {
                    gadget_id: self.base.id,
                    has_focus: false,
                }]
            }

            _ => Vec::new(),
        };

        if self.base.current_value != prev_value {
            messages.push(GadgetMessage::ValueChanged {
                gadget_id: self.base.id,
                value: GadgetValue::Integer(self.base.current_value),
            });
        }

        messages
    }

    fn update(&mut self, delta_time: f32) {
        self.base.update_animation(delta_time);
    }

    fn render(&self, theme: &GadgetTheme) {
        let _commands = self.render_commands(theme);
    }

    fn handle_tab(&mut self, _direction: TabDirection) -> bool {
        true
    }
}

// ============================================================================
// Vertical Slider Implementation
// ============================================================================

/// Vertical slider control for top-to-bottom value selection
pub struct VerticalSlider {
    base: SliderBase,
}

impl VerticalSlider {
    /// Create a new vertical slider
    pub fn new(id: GadgetId, x: i32, y: i32, width: u32, height: u32) -> Self {
        Self {
            base: SliderBase::new(
                id,
                Rect::new(x, y, width, height),
                SliderOrientation::Vertical,
            ),
        }
    }

    /// Set the value range
    pub fn with_range(mut self, min_value: i32, max_value: i32) -> Self {
        self.base.set_range(min_value, max_value);
        self
    }

    /// Set the current value
    pub fn with_value(mut self, value: i32) -> Self {
        self.base.set_value(value);
        self
    }

    /// Set step size for discrete values
    pub fn with_step_size(mut self, step_size: i32) -> Self {
        self.base.set_step_size(Some(step_size));
        self
    }

    /// Enable smooth scrolling animation
    pub fn with_smooth_scrolling(mut self, enabled: bool) -> Self {
        self.base.config.smooth_scrolling = enabled;
        self
    }

    /// Set change callback
    pub fn with_change_callback(mut self, callback: SliderCallback) -> Self {
        self.base.change_callback = Some(callback);
        self
    }

    pub fn set_change_callback<F>(&mut self, callback: F)
    where
        F: Fn(GadgetId, i32) + Send + Sync + 'static,
    {
        self.base.change_callback = Some(Box::new(callback));
    }

    /// Set custom styling
    pub fn with_style(mut self, style: SliderStyle) -> Self {
        self.base.style = style;
        self
    }

    /// Get the current value
    pub fn value(&self) -> i32 {
        self.base.value()
    }

    /// Set the current value
    pub fn set_value(&mut self, value: i32) {
        self.base.set_value(value);
    }

    /// Get the value range
    pub fn range(&self) -> (i32, i32) {
        (self.base.config.min_value, self.base.config.max_value)
    }

    /// Set the value range
    pub fn set_range(&mut self, min_value: i32, max_value: i32) {
        self.base.set_range(min_value, max_value);
    }

    /// Build renderer-facing commands for the current slider state.
    pub fn render_commands(&self, theme: &GadgetTheme) -> Vec<SliderRenderCommand> {
        self.base.render_commands(theme)
    }
}

impl Gadget for VerticalSlider {
    fn id(&self) -> GadgetId {
        self.base.id
    }

    fn bounds(&self) -> Rect {
        self.base.bounds
    }

    fn set_position(&mut self, x: i32, y: i32) {
        self.base.bounds.x = x;
        self.base.bounds.y = y;
    }

    fn set_size(&mut self, width: u32, height: u32) {
        self.base.bounds.width = width;
        self.base.bounds.height = height;
        self.base.update_thumb_position();
    }

    fn state(&self) -> GadgetState {
        self.base.state
    }

    fn is_enabled(&self) -> bool {
        self.base.enabled
    }

    fn set_enabled(&mut self, enabled: bool) {
        self.base.enabled = enabled;
        if !enabled {
            self.base.state = GadgetState::Disabled;
            self.base.thumb_dragging = false;
            self.base.thumb_state = ThumbState::Normal;
        }
    }

    fn is_visible(&self) -> bool {
        self.base.visible
    }

    fn set_visible(&mut self, visible: bool) {
        self.base.visible = visible;
    }

    fn can_focus(&self) -> bool {
        true
    }

    fn has_focus(&self) -> bool {
        self.base.focused
    }

    fn set_focus(&mut self, focused: bool) {
        self.base.focused = focused;
        if focused && self.base.enabled {
            self.base.state = GadgetState::Focused;
        } else if !focused {
            self.base.state = GadgetState::Normal;
        }
    }

    fn handle_input(&mut self, event: &InputEvent) -> Vec<GadgetMessage> {
        if !self.base.enabled || !self.base.visible {
            return Vec::new();
        }

        let prev_value = self.base.current_value;
        let mut messages = match event {
            InputEvent::MouseDown { x, y, button } => self.base.handle_mouse_press(*x, *y, *button),

            InputEvent::MouseUp { x, y, button } => self.base.handle_mouse_release(*x, *y, *button),

            InputEvent::MouseMove { x, y } | InputEvent::MouseDrag { x, y, .. } => {
                self.base.handle_mouse_move(*x, *y)
            }

            InputEvent::MouseEnter { .. } => self.base.handle_mouse_enter(),

            InputEvent::MouseLeave { .. } => self.base.handle_mouse_leave(),

            InputEvent::KeyDown { key, modifiers } => self.base.handle_key_press(*key, *modifiers),

            InputEvent::FocusGained => {
                self.set_focus(true);
                vec![GadgetMessage::FocusChanged {
                    gadget_id: self.base.id,
                    has_focus: true,
                }]
            }

            InputEvent::FocusLost => {
                self.set_focus(false);
                vec![GadgetMessage::FocusChanged {
                    gadget_id: self.base.id,
                    has_focus: false,
                }]
            }

            _ => Vec::new(),
        };

        if self.base.current_value != prev_value {
            messages.push(GadgetMessage::ValueChanged {
                gadget_id: self.base.id,
                value: GadgetValue::Integer(self.base.current_value),
            });
        }

        messages
    }

    fn update(&mut self, delta_time: f32) {
        self.base.update_animation(delta_time);
    }

    fn render(&self, theme: &GadgetTheme) {
        let _commands = self.render_commands(theme);
    }

    fn handle_tab(&mut self, _direction: TabDirection) -> bool {
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_horizontal_slider_creation() {
        let slider = HorizontalSlider::new(1, 10, 20, 200, 20)
            .with_range(0, 100)
            .with_value(50);

        assert_eq!(slider.value(), 50);
        assert_eq!(slider.range(), (0, 100));
        assert_eq!(slider.bounds(), Rect::new(10, 20, 200, 20));
    }

    #[test]
    fn test_value_clamping() {
        let mut slider = HorizontalSlider::new(1, 0, 0, 100, 20).with_range(10, 90);

        slider.set_value(-5); // Below minimum
        assert_eq!(slider.value(), 10);

        slider.set_value(150); // Above maximum
        assert_eq!(slider.value(), 90);

        slider.set_value(50); // Within range
        assert_eq!(slider.value(), 50);
    }

    #[test]
    fn test_step_size() {
        let mut slider = HorizontalSlider::new(1, 0, 0, 100, 20)
            .with_range(0, 100)
            .with_step_size(10);

        slider.set_value(23); // Should snap to nearest step
        assert_eq!(slider.value(), 20);

        slider.set_value(27); // Should snap to nearest step
        assert_eq!(slider.value(), 30);
    }

    #[test]
    fn test_pixel_to_value_conversion() {
        let slider = HorizontalSlider::new(1, 0, 0, 200, 20).with_range(0, 100);

        // Test conversion (approximate due to thumb size)
        let track_length = slider.base.get_track_length();
        let mid_pixel = track_length / 2;
        let mid_value = slider.base.pixel_to_value(mid_pixel);

        // Should be approximately in the middle of the range
        assert!((mid_value - 50).abs() <= 2);
    }

    #[test]
    fn test_vertical_slider() {
        let slider = VerticalSlider::new(1, 10, 20, 20, 200)
            .with_range(0, 255)
            .with_value(128);

        assert_eq!(slider.value(), 128);
        assert_eq!(slider.range(), (0, 255));
        assert_eq!(slider.base.orientation, SliderOrientation::Vertical);
    }

    #[test]
    fn horizontal_render_commands_cover_track_fill_thumb_focus_and_steps() {
        let theme = GadgetTheme::default();
        let mut slider = HorizontalSlider::new(1, 10, 20, 200, 20)
            .with_range(0, 100)
            .with_value(50)
            .with_step_size(5);
        slider.set_focus(true);

        assert_eq!(
            slider.render_commands(&theme),
            vec![
                SliderRenderCommand::Track {
                    rect: Rect::new(10, 28, 200, 4),
                    color: SliderStyle::default().track_color,
                    border_color: SliderStyle::default().track_border_color,
                    border_width: 1,
                },
                SliderRenderCommand::Fill {
                    rect: Rect::new(10, 28, 100, 4),
                    color: SliderStyle::default().track_fill_color,
                },
                SliderRenderCommand::Thumb {
                    rect: Rect::new(102, 20, 16, 20),
                    color: SliderStyle::default().thumb_normal_color,
                    border_color: SliderStyle::default().thumb_border_color,
                    border_width: 1,
                },
                SliderRenderCommand::FocusOutline {
                    rect: Rect::new(10, 20, 200, 20),
                    color: theme.focused_color,
                },
                SliderRenderCommand::StepTicks { step_size: 5 },
            ]
        );
    }

    #[test]
    fn vertical_render_commands_use_same_thumb_geometry_and_skip_hidden() {
        let theme = GadgetTheme::default();
        let mut slider = VerticalSlider::new(1, 30, 40, 20, 200)
            .with_range(0, 100)
            .with_value(50);

        assert_eq!(
            slider.render_commands(&theme),
            vec![
                SliderRenderCommand::Track {
                    rect: Rect::new(38, 40, 4, 200),
                    color: SliderStyle::default().track_color,
                    border_color: SliderStyle::default().track_border_color,
                    border_width: 1,
                },
                SliderRenderCommand::Fill {
                    rect: Rect::new(38, 40, 4, 100),
                    color: SliderStyle::default().track_fill_color,
                },
                SliderRenderCommand::Thumb {
                    rect: Rect::new(32, 130, 16, 20),
                    color: SliderStyle::default().thumb_normal_color,
                    border_color: SliderStyle::default().thumb_border_color,
                    border_width: 1,
                },
            ]
        );

        slider.set_visible(false);
        assert!(slider.render_commands(&theme).is_empty());
    }

    #[test]
    fn test_horizontal_slider_keyboard_matches_cpp_axis() {
        let mut slider = HorizontalSlider::new(1, 0, 0, 100, 20)
            .with_range(0, 10)
            .with_value(5)
            .with_step_size(1);
        slider.set_focus(true);

        let messages = slider.handle_input(&InputEvent::KeyDown {
            key: KeyCode::Right,
            modifiers: KeyModifiers::none(),
        });
        assert_eq!(slider.value(), 3);
        assert!(matches!(
            messages.as_slice(),
            [GadgetMessage::ValueChanged {
                gadget_id: 1,
                value: GadgetValue::Integer(3)
            }]
        ));

        let messages = slider.handle_input(&InputEvent::KeyDown {
            key: KeyCode::Left,
            modifiers: KeyModifiers::none(),
        });
        assert_eq!(slider.value(), 5);
        assert!(matches!(
            messages.as_slice(),
            [GadgetMessage::ValueChanged {
                gadget_id: 1,
                value: GadgetValue::Integer(5)
            }]
        ));
    }

    #[test]
    fn test_vertical_slider_keyboard_matches_cpp_axis() {
        let mut slider = VerticalSlider::new(1, 0, 0, 20, 100)
            .with_range(0, 10)
            .with_value(5)
            .with_step_size(1);
        slider.set_focus(true);

        let messages = slider.handle_input(&InputEvent::KeyDown {
            key: KeyCode::Up,
            modifiers: KeyModifiers::none(),
        });
        assert_eq!(slider.value(), 7);
        assert!(matches!(
            messages.as_slice(),
            [GadgetMessage::ValueChanged {
                gadget_id: 1,
                value: GadgetValue::Integer(7)
            }]
        ));

        let messages = slider.handle_input(&InputEvent::KeyDown {
            key: KeyCode::Down,
            modifiers: KeyModifiers::none(),
        });
        assert_eq!(slider.value(), 5);
        assert!(matches!(
            messages.as_slice(),
            [GadgetMessage::ValueChanged {
                gadget_id: 1,
                value: GadgetValue::Integer(5)
            }]
        ));
    }

    #[test]
    fn test_thumb_bounds() {
        let slider = HorizontalSlider::new(1, 100, 100, 200, 20);
        let thumb_bounds = slider.base.get_thumb_bounds();

        // Thumb should be positioned within slider bounds
        assert!(thumb_bounds.x >= slider.bounds().x);
        assert!(
            thumb_bounds.x + thumb_bounds.width as i32
                <= slider.bounds().x + slider.bounds().width as i32
        );
        assert!(thumb_bounds.y >= slider.bounds().y);
        assert!(
            thumb_bounds.y + thumb_bounds.height as i32
                <= slider.bounds().y + slider.bounds().height as i32
        );
    }

    #[test]
    fn test_range_updates() {
        let mut slider = HorizontalSlider::new(1, 0, 0, 100, 20)
            .with_range(0, 100)
            .with_value(150); // Beyond initial max

        assert_eq!(slider.value(), 100); // Should be clamped to max

        // Update range to accommodate the value
        slider.set_range(0, 200);
        slider.set_value(150);
        assert_eq!(slider.value(), 150); // Now within range
    }
}
