//! CheckBox UI Gadget
//!
//! Interactive checkbox control with checked/unchecked states.

use super::*;

/// Callback function for checkbox state changes
pub type CheckBoxCallback = Box<dyn Fn(bool) + Send + Sync>;

/// CheckBox visual style
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CheckBoxStyle {
    /// Standard checkbox with check mark
    Standard,
    /// Toggle switch style
    Toggle,
    /// Custom style
    Custom,
}

/// CheckBox gadget configuration
#[derive(Debug, Clone)]
pub struct CheckBoxConfig {
    pub style: CheckBoxStyle,
    pub checked_color: Color,
    pub unchecked_color: Color,
    pub check_mark_color: Color,
    pub label: String,
    pub label_offset: i32,
}

impl Default for CheckBoxConfig {
    fn default() -> Self {
        Self {
            style: CheckBoxStyle::Standard,
            checked_color: Color::rgb(50, 150, 250),
            unchecked_color: Color::rgb(200, 200, 200),
            check_mark_color: Color::WHITE,
            label: String::new(),
            label_offset: 5,
        }
    }
}

/// CheckBox UI gadget
pub struct CheckBox {
    id: GadgetId,
    bounds: Rect,
    state: GadgetState,
    enabled: bool,
    visible: bool,
    focused: bool,
    checked: bool,
    mouse_inside: bool,
    config: CheckBoxConfig,
    callback: Option<CheckBoxCallback>,
    tooltip: Option<String>,
    animation_time: f32,
}

impl CheckBox {
    /// Create a new checkbox
    pub fn new(id: GadgetId, x: i32, y: i32, size: u32) -> Self {
        Self {
            id,
            bounds: Rect::new(x, y, size, size),
            state: GadgetState::Normal,
            enabled: true,
            visible: true,
            focused: false,
            checked: false,
            mouse_inside: false,
            config: CheckBoxConfig::default(),
            callback: None,
            tooltip: None,
            animation_time: 0.0,
        }
    }

    /// Set configuration
    pub fn with_config(mut self, config: CheckBoxConfig) -> Self {
        self.config = config;
        self
    }

    /// Set label text
    pub fn with_label(mut self, label: impl Into<String>) -> Self {
        self.config.label = label.into();
        self
    }

    /// Set checked state
    pub fn with_checked(mut self, checked: bool) -> Self {
        self.checked = checked;
        self
    }

    /// Set callback
    pub fn with_callback<F>(mut self, callback: F) -> Self
    where
        F: Fn(bool) + Send + Sync + 'static,
    {
        self.callback = Some(Box::new(callback));
        self
    }

    /// Set tooltip
    pub fn with_tooltip(mut self, tooltip: impl Into<String>) -> Self {
        self.tooltip = Some(tooltip.into());
        self
    }

    /// Set style
    pub fn with_style(mut self, style: CheckBoxStyle) -> Self {
        self.config.style = style;
        self
    }

    /// Get checked state
    pub fn is_checked(&self) -> bool {
        self.checked
    }

    /// Set checked state
    pub fn set_checked(&mut self, checked: bool) {
        if self.checked != checked {
            self.checked = checked;
            self.animation_time = 0.0;
        }

        if let Some(callback) = &self.callback {
            callback(checked);
        }
    }

    /// Toggle checked state
    pub fn toggle(&mut self) {
        self.set_checked(!self.checked);
    }

    /// Get label
    pub fn label(&self) -> &str {
        &self.config.label
    }

    /// Set label
    pub fn set_label(&mut self, label: impl Into<String>) {
        self.config.label = label.into();
    }

    /// Handle mouse click
    fn handle_click(&mut self) -> Vec<GadgetMessage> {
        if self.enabled {
            self.toggle();
            vec![GadgetMessage::Clicked { gadget_id: self.id }]
        } else {
            Vec::new()
        }
    }

    /// Render the checkbox (implementation-specific)
    fn render_checkbox(&self, theme: &GadgetTheme) {
        // Get colors based on state
        let bg_color = if !self.enabled {
            theme.disabled_color
        } else {
            match self.state {
                GadgetState::Normal => {
                    if self.checked {
                        self.config.checked_color
                    } else {
                        self.config.unchecked_color
                    }
                }
                GadgetState::Hovered => {
                    let base = if self.checked {
                        self.config.checked_color
                    } else {
                        self.config.unchecked_color
                    };
                    base.lighten(20)
                }
                GadgetState::Pressed => {
                    let base = if self.checked {
                        self.config.checked_color
                    } else {
                        self.config.unchecked_color
                    };
                    base.darken(20)
                }
                GadgetState::Focused => theme.focused_color,
                GadgetState::Disabled => theme.disabled_color,
            }
        };

        // Animation factor (0.0 to 1.0)
        let anim_factor = (self.animation_time * 5.0).min(1.0);

        // Draw based on style
        match self.config.style {
            CheckBoxStyle::Standard => {
                // Draw box background
                // [Rendering code would go here]

                // Draw check mark if checked
                if self.checked {
                    let check_alpha = if self.animation_time < 0.2 {
                        anim_factor
                    } else {
                        1.0
                    };
                    // Draw check mark with alpha
                    // [Rendering code would go here]
                }

                // Draw border
                if self.focused {
                    // Draw focus border
                    // [Rendering code would go here]
                }
            }
            CheckBoxStyle::Toggle => {
                // Draw toggle switch background
                let toggle_width = self.bounds.width as f32 * 1.5;
                let toggle_height = self.bounds.height as f32;

                // Background pill shape
                // [Rendering code would go here]

                // Toggle circle
                let circle_x = if self.checked {
                    toggle_width * 0.7 * anim_factor + toggle_width * 0.3 * (1.0 - anim_factor)
                } else {
                    toggle_width * 0.3 * anim_factor + toggle_width * 0.7 * (1.0 - anim_factor)
                };
                // [Rendering code would go here]
            }
            CheckBoxStyle::Custom => {
                // Custom rendering
                // [Rendering code would go here]
            }
        }

        // Draw label if present
        if !self.config.label.is_empty() {
            let label_x = self.bounds.x + self.bounds.width as i32 + self.config.label_offset;
            let label_y = self.bounds.y + (self.bounds.height / 2) as i32;

            let text_color = if self.enabled {
                theme.text_color
            } else {
                theme.disabled_text_color
            };

            // [Text rendering code would go here]
        }
    }
}

impl Gadget for CheckBox {
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
        } else if self.state == GadgetState::Disabled {
            self.state = GadgetState::Normal;
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
        if focused {
            self.state = GadgetState::Hovered;
        } else if self.state == GadgetState::Focused || self.state == GadgetState::Hovered {
            self.state = GadgetState::Normal;
        }
    }

    fn handle_input(&mut self, event: &InputEvent) -> Vec<GadgetMessage> {
        if !self.enabled || !self.visible {
            return Vec::new();
        }

        match event {
            InputEvent::MouseEnter { .. } => {
                self.mouse_inside = true;
                if self.enabled {
                    self.state = GadgetState::Hovered;
                }
                return vec![GadgetMessage::MouseEnter { gadget_id: self.id }];
            }

            InputEvent::MouseLeave { .. } => {
                self.mouse_inside = false;
                if self.enabled {
                    self.state = if self.focused {
                        GadgetState::Hovered
                    } else {
                        GadgetState::Normal
                    };
                }
                return vec![GadgetMessage::MouseLeave { gadget_id: self.id }];
            }

            InputEvent::MouseDrag { button, .. } => {
                if *button == MouseButton::Left {
                    return vec![GadgetMessage::LeftDrag { gadget_id: self.id }];
                }
            }

            InputEvent::MouseDown { button, .. } => {
                if matches!(button, MouseButton::Left | MouseButton::Right) {
                    // C++ handles left/right down without changing selection.
                    return vec![GadgetMessage::Custom {
                        gadget_id: self.id,
                        data: "input_handled".to_string(),
                    }];
                }
            }

            InputEvent::MouseUp { button, .. } => {
                if *button == MouseButton::Left {
                    if !self.mouse_inside {
                        return Vec::new();
                    }
                    return self.handle_click();
                }
                if *button == MouseButton::Right {
                    if self.checked {
                        self.checked = false;
                        self.animation_time = 0.0;
                        return vec![GadgetMessage::RightClicked { gadget_id: self.id }];
                    } else {
                        return Vec::new();
                    }
                }
            }

            InputEvent::KeyDown { key, .. } => {
                if self.focused {
                    match key {
                        KeyCode::Space | KeyCode::Enter => {
                            return self.handle_click();
                        }
                        KeyCode::Tab | KeyCode::Right | KeyCode::Down => {
                            return vec![GadgetMessage::Custom {
                                gadget_id: self.id,
                                data: "tab_next".to_string(),
                            }];
                        }
                        KeyCode::Left | KeyCode::Up => {
                            return vec![GadgetMessage::Custom {
                                gadget_id: self.id,
                                data: "tab_prev".to_string(),
                            }];
                        }
                        _ => {}
                    }
                }
            }

            InputEvent::FocusGained => {
                self.set_focus(true);
                return vec![GadgetMessage::FocusChanged {
                    gadget_id: self.id,
                    has_focus: true,
                }];
            }

            InputEvent::FocusLost => {
                self.set_focus(false);
                return vec![GadgetMessage::FocusChanged {
                    gadget_id: self.id,
                    has_focus: false,
                }];
            }

            _ => {}
        }

        Vec::new()
    }

    fn update(&mut self, delta_time: f32) {
        self.animation_time += delta_time;
    }

    fn render(&self, theme: &GadgetTheme) {
        if !self.visible {
            return;
        }

        self.render_checkbox(theme);
    }

    fn tooltip(&self) -> Option<&str> {
        self.tooltip.as_deref()
    }
}

/// Builder for creating checkboxes
pub struct CheckBoxBuilder {
    id: GadgetId,
    x: i32,
    y: i32,
    size: u32,
    config: CheckBoxConfig,
    checked: bool,
    callback: Option<CheckBoxCallback>,
    tooltip: Option<String>,
}

impl CheckBoxBuilder {
    pub fn new(id: GadgetId, x: i32, y: i32, size: u32) -> Self {
        Self {
            id,
            x,
            y,
            size,
            config: CheckBoxConfig::default(),
            checked: false,
            callback: None,
            tooltip: None,
        }
    }

    pub fn label(mut self, label: impl Into<String>) -> Self {
        self.config.label = label.into();
        self
    }

    pub fn checked(mut self, checked: bool) -> Self {
        self.checked = checked;
        self
    }

    pub fn style(mut self, style: CheckBoxStyle) -> Self {
        self.config.style = style;
        self
    }

    pub fn callback<F>(mut self, callback: F) -> Self
    where
        F: Fn(bool) + Send + Sync + 'static,
    {
        self.callback = Some(Box::new(callback));
        self
    }

    pub fn tooltip(mut self, tooltip: impl Into<String>) -> Self {
        self.tooltip = Some(tooltip.into());
        self
    }

    pub fn build(self) -> CheckBox {
        let mut checkbox = CheckBox::new(self.id, self.x, self.y, self.size)
            .with_config(self.config)
            .with_checked(self.checked);

        if let Some(callback) = self.callback {
            checkbox.callback = Some(callback);
        }

        if let Some(tooltip) = self.tooltip {
            checkbox.tooltip = Some(tooltip);
        }

        checkbox
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    };

    #[test]
    fn test_checkbox_creation() {
        let checkbox = CheckBox::new(1, 10, 20, 20);
        assert_eq!(checkbox.id(), 1);
        assert!(!checkbox.is_checked());
        assert!(checkbox.is_enabled());
    }

    #[test]
    fn test_checkbox_toggle() {
        let mut checkbox = CheckBox::new(1, 10, 20, 20);
        assert!(!checkbox.is_checked());

        checkbox.toggle();
        assert!(checkbox.is_checked());

        checkbox.toggle();
        assert!(!checkbox.is_checked());
    }

    #[test]
    fn test_checkbox_with_label() {
        let checkbox = CheckBox::new(1, 10, 20, 20).with_label("Test Label");
        assert_eq!(checkbox.label(), "Test Label");
    }

    #[test]
    fn test_checkbox_builder() {
        let checkbox = CheckBoxBuilder::new(1, 10, 20, 20)
            .label("Builder Test")
            .checked(true)
            .style(CheckBoxStyle::Toggle)
            .build();

        assert_eq!(checkbox.label(), "Builder Test");
        assert!(checkbox.is_checked());
        assert_eq!(checkbox.config.style, CheckBoxStyle::Toggle);
    }

    #[test]
    fn test_checkbox_input() {
        let mut checkbox = CheckBox::new(1, 10, 20, 20);

        // Test mouse hover
        let messages = checkbox.handle_input(&InputEvent::MouseEnter { x: 15, y: 25 });
        assert_eq!(checkbox.state(), GadgetState::Hovered);

        // Test click
        checkbox.handle_input(&InputEvent::MouseDown {
            x: 15,
            y: 25,
            button: MouseButton::Left,
        });

        let messages = checkbox.handle_input(&InputEvent::MouseUp {
            x: 15,
            y: 25,
            button: MouseButton::Left,
        });
        assert!(checkbox.is_checked());
        assert!(!messages.is_empty());
    }

    #[test]
    fn right_click_clears_checked_without_normal_callback_like_cpp() {
        let callback_count = Arc::new(AtomicUsize::new(0));
        let callback_count_for_closure = Arc::clone(&callback_count);
        let mut checkbox = CheckBox::new(1, 10, 20, 20)
            .with_checked(true)
            .with_callback(move |_| {
                callback_count_for_closure.fetch_add(1, Ordering::SeqCst);
            });

        let messages = checkbox.handle_input(&InputEvent::MouseUp {
            x: 15,
            y: 25,
            button: MouseButton::Right,
        });

        assert!(!checkbox.is_checked());
        assert_eq!(callback_count.load(Ordering::SeqCst), 0);
        assert!(matches!(
            messages.as_slice(),
            [GadgetMessage::RightClicked { gadget_id: 1 }]
        ));
    }

    #[test]
    fn set_checked_notifies_even_when_state_is_unchanged_like_cpp() {
        let callback_count = Arc::new(AtomicUsize::new(0));
        let callback_count_for_closure = Arc::clone(&callback_count);
        let mut checkbox = CheckBox::new(1, 10, 20, 20).with_callback(move |_| {
            callback_count_for_closure.fetch_add(1, Ordering::SeqCst);
        });

        checkbox.set_checked(true);
        checkbox.set_checked(true);

        assert!(checkbox.is_checked());
        assert_eq!(callback_count.load(Ordering::SeqCst), 2);
    }

    #[test]
    fn keyboard_arrows_and_tab_request_focus_navigation_like_cpp() {
        let mut checkbox = CheckBox::new(1, 10, 20, 20);
        checkbox.set_focus(true);

        let next = checkbox.handle_input(&InputEvent::KeyDown {
            key: KeyCode::Tab,
            modifiers: KeyModifiers::none(),
        });
        assert!(matches!(
            next.as_slice(),
            [GadgetMessage::Custom { gadget_id: 1, data } ] if data == "tab_next"
        ));

        let prev = checkbox.handle_input(&InputEvent::KeyDown {
            key: KeyCode::Left,
            modifiers: KeyModifiers::none(),
        });
        assert!(matches!(
            prev.as_slice(),
            [GadgetMessage::Custom { gadget_id: 1, data } ] if data == "tab_prev"
        ));
    }

    #[test]
    fn mouse_down_is_handled_without_toggling_like_cpp() {
        let mut checkbox = CheckBox::new(1, 10, 20, 20);

        let left = checkbox.handle_input(&InputEvent::MouseDown {
            x: 12,
            y: 22,
            button: MouseButton::Left,
        });
        assert!(!checkbox.is_checked());
        assert!(matches!(
            left.as_slice(),
            [GadgetMessage::Custom { gadget_id: 1, data } ] if data == "input_handled"
        ));

        let right = checkbox.handle_input(&InputEvent::MouseDown {
            x: 12,
            y: 22,
            button: MouseButton::Right,
        });
        assert!(!checkbox.is_checked());
        assert!(matches!(
            right.as_slice(),
            [GadgetMessage::Custom { gadget_id: 1, data } ] if data == "input_handled"
        ));
    }
}
