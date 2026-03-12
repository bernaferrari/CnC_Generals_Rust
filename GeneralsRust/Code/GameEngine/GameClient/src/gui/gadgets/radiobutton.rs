//! RadioButton UI Gadget
//!
//! Mutually exclusive radio button controls in groups.

use super::*;
use std::sync::{Arc, Mutex};

/// Radio button group ID
pub type RadioGroupId = u32;

/// Callback function for radio button selection
pub type RadioButtonCallback = Box<dyn Fn(GadgetId) + Send + Sync>;

/// Radio button group for mutual exclusivity
#[derive(Clone)]
pub struct RadioButtonGroup {
    id: RadioGroupId,
    selected: Arc<Mutex<Option<GadgetId>>>,
    buttons: Arc<Mutex<Vec<GadgetId>>>,
}

impl RadioButtonGroup {
    /// Create a new radio button group
    pub fn new(id: RadioGroupId) -> Self {
        Self {
            id,
            selected: Arc::new(Mutex::new(None)),
            buttons: Arc::new(Mutex::new(Vec::new())),
        }
    }

    /// Add a button to the group
    fn add_button(&self, button_id: GadgetId) {
        let mut buttons = self.buttons.lock().unwrap();
        if !buttons.contains(&button_id) {
            buttons.push(button_id);
        }
    }

    /// Remove a button from the group
    fn remove_button(&self, button_id: GadgetId) {
        let mut buttons = self.buttons.lock().unwrap();
        buttons.retain(|&id| id != button_id);

        let mut selected = self.selected.lock().unwrap();
        if *selected == Some(button_id) {
            *selected = None;
        }
    }

    /// Set selected button
    fn set_selected(&self, button_id: GadgetId) {
        let mut selected = self.selected.lock().unwrap();
        *selected = Some(button_id);
    }

    /// Get selected button
    pub fn get_selected(&self) -> Option<GadgetId> {
        *self.selected.lock().unwrap()
    }

    /// Get all buttons in group
    pub fn get_buttons(&self) -> Vec<GadgetId> {
        self.buttons.lock().unwrap().clone()
    }

    /// Check if a button is selected
    pub fn is_selected(&self, button_id: GadgetId) -> bool {
        *self.selected.lock().unwrap() == Some(button_id)
    }

    /// Clear selection
    pub fn clear_selection(&self) {
        *self.selected.lock().unwrap() = None;
    }
}

/// RadioButton gadget
pub struct RadioButton {
    id: GadgetId,
    bounds: Rect,
    state: GadgetState,
    enabled: bool,
    visible: bool,
    focused: bool,
    selected: bool,
    mouse_inside: bool,
    label: String,
    label_offset: i32,
    group: RadioButtonGroup,
    callback: Option<RadioButtonCallback>,
    tooltip: Option<String>,
    animation_time: f32,
    selected_color: Color,
    unselected_color: Color,
    dot_color: Color,
}

impl RadioButton {
    /// Create a new radio button
    pub fn new(id: GadgetId, x: i32, y: i32, size: u32, group: RadioButtonGroup) -> Self {
        group.add_button(id);

        Self {
            id,
            bounds: Rect::new(x, y, size, size),
            state: GadgetState::Normal,
            enabled: true,
            visible: true,
            focused: false,
            selected: false,
            mouse_inside: false,
            label: String::new(),
            label_offset: 5,
            group,
            callback: None,
            tooltip: None,
            animation_time: 0.0,
            selected_color: Color::rgb(50, 150, 250),
            unselected_color: Color::rgb(200, 200, 200),
            dot_color: Color::WHITE,
        }
    }

    /// Set label text
    pub fn with_label(mut self, label: impl Into<String>) -> Self {
        self.label = label.into();
        self
    }

    /// Set selected state
    pub fn with_selected(mut self, selected: bool) -> Self {
        if selected {
            self.select();
        }
        self
    }

    /// Set callback
    pub fn with_callback<F>(mut self, callback: F) -> Self
    where
        F: Fn(GadgetId) + Send + Sync + 'static,
    {
        self.callback = Some(Box::new(callback));
        self
    }

    /// Set tooltip
    pub fn with_tooltip(mut self, tooltip: impl Into<String>) -> Self {
        self.tooltip = Some(tooltip.into());
        self
    }

    /// Set colors
    pub fn with_colors(mut self, selected: Color, unselected: Color, dot: Color) -> Self {
        self.selected_color = selected;
        self.unselected_color = unselected;
        self.dot_color = dot;
        self
    }

    /// Check if selected
    pub fn is_selected(&self) -> bool {
        self.selected
    }

    /// Select this radio button
    pub fn select(&mut self) {
        if !self.selected {
            self.selected = true;
            self.group.set_selected(self.id);
            self.animation_time = 0.0;

            if let Some(callback) = &self.callback {
                callback(self.id);
            }
        }
    }

    /// Deselect this radio button
    fn deselect(&mut self) {
        self.selected = false;
    }

    /// Get label
    pub fn label(&self) -> &str {
        &self.label
    }

    /// Set label
    pub fn set_label(&mut self, label: impl Into<String>) {
        self.label = label.into();
    }

    /// Get group
    pub fn group(&self) -> &RadioButtonGroup {
        &self.group
    }

    /// Handle mouse click
    fn handle_click(&mut self) -> Vec<GadgetMessage> {
        if self.enabled && !self.selected {
            self.select();
            vec![GadgetMessage::Clicked { gadget_id: self.id }]
        } else {
            Vec::new()
        }
    }

    /// Render the radio button
    #[allow(unused_variables)]
    fn render_radiobutton(&self, theme: &GadgetTheme) {
        // Get colors based on state
        let bg_color = if !self.enabled {
            theme.disabled_color
        } else {
            match self.state {
                GadgetState::Normal => {
                    if self.selected {
                        self.selected_color
                    } else {
                        self.unselected_color
                    }
                }
                GadgetState::Hovered => {
                    let base = if self.selected {
                        self.selected_color
                    } else {
                        self.unselected_color
                    };
                    base.lighten(20)
                }
                GadgetState::Pressed => {
                    let base = if self.selected {
                        self.selected_color
                    } else {
                        self.unselected_color
                    };
                    base.darken(20)
                }
                GadgetState::Focused => theme.focused_color,
                GadgetState::Disabled => theme.disabled_color,
            }
        };

        // Animation factor (0.0 to 1.0)
        let anim_factor = (self.animation_time * 5.0).min(1.0);

        // Draw outer circle
        let center_x = self.bounds.x + (self.bounds.width / 2) as i32;
        let center_y = self.bounds.y + (self.bounds.height / 2) as i32;
        let radius = (self.bounds.width / 2) as f32;

        // [Circle rendering code would go here]

        // Draw inner dot if selected
        if self.selected {
            let dot_radius = radius * 0.5 * anim_factor;
            let dot_alpha = anim_factor;

            // [Inner dot rendering code would go here]
        }

        // Draw focus ring
        if self.focused {
            let focus_radius = radius + 3.0;
            // [Focus ring rendering code would go here]
        }

        // Draw label if present
        if !self.label.is_empty() {
            let label_x = self.bounds.x + self.bounds.width as i32 + self.label_offset;
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

impl Gadget for RadioButton {
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
        if !focused && !self.mouse_inside && self.state == GadgetState::Hovered {
            self.state = GadgetState::Normal;
        }
    }

    fn handle_input(&mut self, event: &InputEvent) -> Vec<GadgetMessage> {
        if !self.enabled || !self.visible {
            return Vec::new();
        }

        // Sync selected state with group
        let group_selected = self.group.get_selected();
        if group_selected != Some(self.id) && self.selected {
            self.deselect();
        } else if group_selected == Some(self.id) && !self.selected {
            self.selected = true;
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
                    self.state = GadgetState::Normal;
                }
                return vec![GadgetMessage::MouseLeave { gadget_id: self.id }];
            }

            InputEvent::MouseDrag { button, .. } => {
                if *button == MouseButton::Left {
                    return vec![GadgetMessage::LeftDrag { gadget_id: self.id }];
                }
            }

            InputEvent::MouseDown { button, .. } => {
                if *button == MouseButton::Left {
                    // C++: no action on left down for radio buttons.
                }
            }

            InputEvent::MouseUp { button, .. } => {
                if *button == MouseButton::Left {
                    if !self.mouse_inside {
                        return Vec::new();
                    }
                    return self.handle_click();
                }
            }

            InputEvent::KeyDown { key, .. } => {
                if self.focused {
                    match key {
                        KeyCode::Space | KeyCode::Enter => {
                            return self.handle_click();
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

        // Sync with group
        let group_selected = self.group.get_selected();
        if group_selected != Some(self.id) && self.selected {
            self.deselect();
        } else if group_selected == Some(self.id) && !self.selected {
            self.selected = true;
        }
    }

    fn render(&self, theme: &GadgetTheme) {
        if !self.visible {
            return;
        }

        self.render_radiobutton(theme);
    }

    fn tooltip(&self) -> Option<&str> {
        self.tooltip.as_deref()
    }
}

impl Drop for RadioButton {
    fn drop(&mut self) {
        self.group.remove_button(self.id);
    }
}

/// Builder for creating radio buttons
pub struct RadioButtonBuilder {
    id: GadgetId,
    x: i32,
    y: i32,
    size: u32,
    group: RadioButtonGroup,
    label: Option<String>,
    selected: bool,
    callback: Option<RadioButtonCallback>,
    tooltip: Option<String>,
}

impl RadioButtonBuilder {
    pub fn new(id: GadgetId, x: i32, y: i32, size: u32, group: RadioButtonGroup) -> Self {
        Self {
            id,
            x,
            y,
            size,
            group,
            label: None,
            selected: false,
            callback: None,
            tooltip: None,
        }
    }

    pub fn label(mut self, label: impl Into<String>) -> Self {
        self.label = Some(label.into());
        self
    }

    pub fn selected(mut self, selected: bool) -> Self {
        self.selected = selected;
        self
    }

    pub fn callback<F>(mut self, callback: F) -> Self
    where
        F: Fn(GadgetId) + Send + Sync + 'static,
    {
        self.callback = Some(Box::new(callback));
        self
    }

    pub fn tooltip(mut self, tooltip: impl Into<String>) -> Self {
        self.tooltip = Some(tooltip.into());
        self
    }

    pub fn build(self) -> RadioButton {
        let mut radio = RadioButton::new(self.id, self.x, self.y, self.size, self.group);

        if let Some(label) = self.label {
            radio.label = label;
        }

        if self.selected {
            radio.select();
        }

        if let Some(callback) = self.callback {
            radio.callback = Some(callback);
        }

        if let Some(tooltip) = self.tooltip {
            radio.tooltip = Some(tooltip);
        }

        radio
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_radio_group() {
        let group = RadioButtonGroup::new(1);
        assert_eq!(group.get_selected(), None);
        assert_eq!(group.get_buttons().len(), 0);
    }

    #[test]
    fn test_radio_button_creation() {
        let group = RadioButtonGroup::new(1);
        let radio = RadioButton::new(1, 10, 20, 20, group.clone());

        assert_eq!(radio.id(), 1);
        assert!(!radio.is_selected());
        assert_eq!(group.get_buttons().len(), 1);
    }

    #[test]
    fn test_radio_button_selection() {
        let group = RadioButtonGroup::new(1);
        let mut radio1 = RadioButton::new(1, 10, 20, 20, group.clone());
        let mut radio2 = RadioButton::new(2, 10, 50, 20, group.clone());

        radio1.select();
        assert!(radio1.is_selected());
        assert_eq!(group.get_selected(), Some(1));

        radio2.select();
        radio1.update(0.0); // Sync with group
        assert!(!radio1.is_selected());
        assert!(radio2.is_selected());
        assert_eq!(group.get_selected(), Some(2));
    }

    #[test]
    fn test_radio_button_group_exclusivity() {
        let group = RadioButtonGroup::new(1);
        let mut radio1 = RadioButton::new(1, 10, 20, 20, group.clone());
        let mut radio2 = RadioButton::new(2, 10, 50, 20, group.clone());
        let mut radio3 = RadioButton::new(3, 10, 80, 20, group.clone());

        assert_eq!(group.get_buttons().len(), 3);

        radio1.select();
        assert_eq!(group.get_selected(), Some(1));

        radio2.select();
        radio1.update(0.0);
        assert!(!radio1.is_selected());
        assert!(radio2.is_selected());
        assert!(!radio3.is_selected());

        radio3.select();
        radio1.update(0.0);
        radio2.update(0.0);
        assert!(!radio1.is_selected());
        assert!(!radio2.is_selected());
        assert!(radio3.is_selected());
    }

    #[test]
    fn test_radio_button_builder() {
        let group = RadioButtonGroup::new(1);
        let radio = RadioButtonBuilder::new(1, 10, 20, 20, group.clone())
            .label("Test Option")
            .selected(true)
            .build();

        assert_eq!(radio.label(), "Test Option");
        assert!(radio.is_selected());
    }
}
