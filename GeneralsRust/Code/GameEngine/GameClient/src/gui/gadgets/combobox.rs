//! ComboBox UI Gadget
//!
//! Dropdown list selection control.

use super::*;

/// ComboBox item
#[derive(Debug, Clone)]
pub struct ComboBoxItem {
    pub id: u32,
    pub text: String,
    pub enabled: bool,
    pub icon: Option<String>,
    pub data: Option<i32>,
}

impl ComboBoxItem {
    pub fn new(id: u32, text: impl Into<String>) -> Self {
        Self {
            id,
            text: text.into(),
            enabled: true,
            icon: None,
            data: None,
        }
    }

    pub fn with_data(mut self, data: i32) -> Self {
        self.data = Some(data);
        self
    }
}

/// ComboBox selection callback
pub type ComboBoxCallback = Box<dyn Fn(u32) + Send + Sync>;

/// ComboBox gadget
pub struct ComboBox {
    id: GadgetId,
    bounds: Rect,
    state: GadgetState,
    enabled: bool,
    visible: bool,
    focused: bool,
    items: Vec<ComboBoxItem>,
    selected_index: Option<usize>,
    dropdown_open: bool,
    max_dropdown_height: u32,
    item_height: u32,
    callback: Option<ComboBoxCallback>,
    tooltip: Option<String>,
    hovered_item: Option<usize>,
    text: String,
    is_editable: bool,
    max_chars: usize,
    ascii_only: bool,
    letters_and_numbers: bool,
    dont_hide_next: bool,
}

impl ComboBox {
    /// Create a new combobox
    pub fn new(id: GadgetId, x: i32, y: i32, width: u32, height: u32) -> Self {
        Self {
            id,
            bounds: Rect::new(x, y, width, height),
            state: GadgetState::Normal,
            enabled: true,
            visible: true,
            focused: false,
            items: Vec::new(),
            selected_index: None,
            dropdown_open: false,
            max_dropdown_height: 200,
            item_height: height,
            callback: None,
            tooltip: None,
            hovered_item: None,
            text: String::new(),
            is_editable: false,
            max_chars: 0,
            ascii_only: false,
            letters_and_numbers: false,
            dont_hide_next: false,
        }
    }

    /// Add an item
    pub fn add_item(&mut self, item: ComboBoxItem) {
        self.items.push(item);

        if self.selected_index.is_none() {
            self.selected_index = Some(0);
            if let Some(item) = self.items.first() {
                self.text = item.text.clone();
            }
        }
    }

    pub fn set_item_data(&mut self, index: usize, data: i32) -> bool {
        if let Some(item) = self.items.get_mut(index) {
            item.data = Some(data);
            return true;
        }
        false
    }

    pub fn selected_item_data(&self) -> Option<i32> {
        self.selected_index
            .and_then(|index| self.items.get(index))
            .and_then(|item| item.data)
    }

    pub fn clear(&mut self) {
        self.items.clear();
        self.selected_index = None;
        self.dropdown_open = false;
        self.hovered_item = None;
        self.text.clear();
    }

    /// Remove an item
    pub fn remove_item(&mut self, index: usize) -> bool {
        if index < self.items.len() {
            self.items.remove(index);

            if self.selected_index == Some(index) {
                self.selected_index = if self.items.is_empty() { None } else { Some(0) };
            }

            true
        } else {
            false
        }
    }

    /// Select item by index
    pub fn select_index(&mut self, index: usize) -> bool {
        if index < self.items.len() && self.items[index].enabled {
            self.selected_index = Some(index);
            self.text = self.items[index].text.clone();

            if let Some(callback) = &self.callback {
                callback(self.items[index].id);
            }

            true
        } else {
            false
        }
    }

    pub fn set_dont_hide_next(&mut self, dont_hide: bool) {
        self.dont_hide_next = dont_hide;
    }

    pub fn take_dont_hide_next(&mut self) -> bool {
        let value = self.dont_hide_next;
        self.dont_hide_next = false;
        value
    }

    /// Select item by ID
    pub fn select_item(&mut self, item_id: u32) -> bool {
        if let Some(index) = self.items.iter().position(|item| item.id == item_id) {
            self.select_index(index)
        } else {
            false
        }
    }

    /// Get selected item
    pub fn selected_item(&self) -> Option<&ComboBoxItem> {
        self.selected_index.and_then(|index| self.items.get(index))
    }

    pub fn selected_id(&self) -> Option<u32> {
        self.selected_item().map(|item| item.id)
    }

    pub fn set_max_display(&mut self, max_display: usize) {
        if max_display > 0 {
            self.max_dropdown_height = self.item_height.saturating_mul(max_display as u32);
        }
    }

    /// Get selected index
    pub fn selected_index(&self) -> Option<usize> {
        self.selected_index
    }

    pub fn set_text(&mut self, text: impl Into<String>) {
        self.text = text.into();
        if let Some(index) = self.items.iter().position(|item| item.text == self.text) {
            self.selected_index = Some(index);
        }
    }

    pub fn text(&self) -> &str {
        if self.text.is_empty() {
            self.selected_item()
                .map(|item| item.text.as_str())
                .unwrap_or("")
        } else {
            &self.text
        }
    }

    pub fn set_editable(&mut self, editable: bool) {
        self.is_editable = editable;
    }

    pub fn set_max_chars(&mut self, max_chars: usize) {
        self.max_chars = max_chars;
    }

    pub fn set_ascii_only(&mut self, ascii_only: bool) {
        self.ascii_only = ascii_only;
    }

    pub fn set_letters_and_numbers(&mut self, letters_only: bool) {
        self.letters_and_numbers = letters_only;
    }

    /// Open dropdown
    pub fn open(&mut self) {
        if self.enabled && !self.items.is_empty() {
            self.dropdown_open = true;
        }
    }

    /// Close dropdown
    pub fn close(&mut self) {
        self.dropdown_open = false;
    }

    /// Toggle dropdown
    pub fn toggle(&mut self) {
        if self.dropdown_open {
            self.close();
        } else {
            self.open();
        }
    }

    /// Is dropdown open
    pub fn is_open(&self) -> bool {
        self.dropdown_open
    }

    pub fn dropdown_bounds(&self) -> Rect {
        self.dropdown_bounds_internal()
    }

    pub fn item_height(&self) -> u32 {
        self.item_height
    }

    pub fn hovered_item(&self) -> Option<usize> {
        self.hovered_item
    }

    pub fn max_display(&self) -> usize {
        if self.item_height == 0 {
            0
        } else {
            (self.max_dropdown_height / self.item_height).max(1) as usize
        }
    }

    /// Get items
    pub fn items(&self) -> &[ComboBoxItem] {
        &self.items
    }

    /// Set callback
    pub fn with_callback<F>(mut self, callback: F) -> Self
    where
        F: Fn(u32) + Send + Sync + 'static,
    {
        self.callback = Some(Box::new(callback));
        self
    }

    /// Get dropdown bounds
    fn dropdown_bounds_internal(&self) -> Rect {
        let dropdown_height =
            (self.items.len() as u32 * self.item_height).min(self.max_dropdown_height);

        Rect::new(
            self.bounds.x,
            self.bounds.y + self.bounds.height as i32,
            self.bounds.width,
            dropdown_height,
        )
    }

    /// Find item at position in dropdown
    fn item_at_position(&self, x: i32, y: i32) -> Option<usize> {
        let dropdown = self.dropdown_bounds_internal();

        if !dropdown.contains_point(x, y) {
            return None;
        }

        let relative_y = (y - dropdown.y) as u32;
        let index = (relative_y / self.item_height) as usize;

        if index < self.items.len() {
            Some(index)
        } else {
            None
        }
    }
}

impl Gadget for ComboBox {
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
            self.close();
        }
    }

    fn is_visible(&self) -> bool {
        self.visible
    }

    fn set_visible(&mut self, visible: bool) {
        self.visible = visible;
        if !visible {
            self.close();
        }
    }

    fn can_focus(&self) -> bool {
        true
    }

    fn has_focus(&self) -> bool {
        self.focused
    }

    fn set_focus(&mut self, focused: bool) {
        self.focused = focused;
        if !focused {
            self.close();
        }
    }

    fn handle_input(&mut self, event: &InputEvent) -> Vec<GadgetMessage> {
        if !self.enabled || !self.visible {
            return Vec::new();
        }

        match event {
            InputEvent::MouseMove { x, y } => {
                if self.dropdown_open {
                    self.hovered_item = self.item_at_position(*x, *y);
                }
            }

            InputEvent::MouseDown { x, y, button } => {
                if *button == MouseButton::Left {
                    if self.bounds.contains_point(*x, *y) {
                        self.toggle();
                    } else if self.dropdown_open {
                        let dropdown = self.dropdown_bounds_internal();
                        if !dropdown.contains_point(*x, *y) {
                            self.close();
                        }
                    }
                }
            }

            InputEvent::MouseUp { x, y, button } => {
                if *button == MouseButton::Left && self.dropdown_open {
                    if let Some(index) = self.item_at_position(*x, *y) {
                        if self.select_index(index) {
                            self.close();
                            return vec![GadgetMessage::ValueChanged {
                                gadget_id: self.id,
                                value: GadgetValue::Integer(self.items[index].id as i32),
                            }];
                        }
                    }
                }
            }

            InputEvent::KeyDown { key, .. } => {
                if self.focused {
                    match key {
                        KeyCode::Enter | KeyCode::Space => {
                            self.toggle();
                        }
                        KeyCode::Escape => {
                            self.close();
                        }
                        KeyCode::Up => {
                            if let Some(index) = self.selected_index {
                                if index > 0 {
                                    self.select_index(index - 1);
                                }
                            }
                        }
                        KeyCode::Down => {
                            if let Some(index) = self.selected_index {
                                if index + 1 < self.items.len() {
                                    self.select_index(index + 1);
                                }
                            } else if !self.items.is_empty() {
                                self.select_index(0);
                            }
                        }
                        KeyCode::Backspace => {
                            if self.is_editable && !self.text.is_empty() {
                                self.text.pop();
                                return vec![GadgetMessage::ValueChanged {
                                    gadget_id: self.id,
                                    value: GadgetValue::String(self.text.clone()),
                                }];
                            }
                        }
                        KeyCode::Char(ch) => {
                            if self.is_editable {
                                if self.max_chars > 0 && self.text.len() >= self.max_chars {
                                    return Vec::new();
                                }
                                if self.ascii_only && !ch.is_ascii() {
                                    return Vec::new();
                                }
                                if self.letters_and_numbers && !ch.is_ascii_alphanumeric() {
                                    return Vec::new();
                                }
                                self.text.push(*ch);
                                return vec![GadgetMessage::ValueChanged {
                                    gadget_id: self.id,
                                    value: GadgetValue::String(self.text.clone()),
                                }];
                            }
                        }
                        _ => {}
                    }
                }
            }

            _ => {}
        }

        Vec::new()
    }

    fn update(&mut self, _delta_time: f32) {
        // C++ GadgetComboBox is event-driven with no per-frame update.
        // Validate selected index still points at a valid item.
        if let Some(idx) = self.selected_index {
            if idx >= self.items.len() {
                self.selected_index = if self.items.is_empty() { None } else { Some(0) };
                self.text = self
                    .selected_index
                    .and_then(|i| self.items.get(i))
                    .map(|item| item.text.clone())
                    .unwrap_or_default();
            }
        }
        // Close dropdown if all items removed.
        if self.dropdown_open && self.items.is_empty() {
            self.dropdown_open = false;
        }
    }

    fn render(&self, theme: &GadgetTheme) {
        if !self.visible {
            return;
        }

        // Render main box with selected item
        // [Main box rendering code]

        // Render dropdown arrow
        // [Arrow rendering code]

        // Render dropdown list if open
        if self.dropdown_open {
            let dropdown = self.dropdown_bounds_internal();

            // Render dropdown background
            // [Background rendering code]

            // Render items
            for (index, item) in self.items.iter().enumerate() {
                let item_y = dropdown.y + (index as u32 * self.item_height) as i32;
                let is_selected = self.selected_index == Some(index);
                let is_hovered = self.hovered_item == Some(index);

                // [Item rendering code]
            }
        }
    }

    fn tooltip(&self) -> Option<&str> {
        self.tooltip.as_deref()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_combobox_creation() {
        let combobox = ComboBox::new(1, 10, 20, 150, 25);
        assert_eq!(combobox.items().len(), 0);
        assert!(!combobox.is_open());
    }

    #[test]
    fn test_add_items() {
        let mut combobox = ComboBox::new(1, 10, 20, 150, 25);
        combobox.add_item(ComboBoxItem::new(1, "Item 1"));
        combobox.add_item(ComboBoxItem::new(2, "Item 2"));

        assert_eq!(combobox.items().len(), 2);
        assert_eq!(combobox.selected_index(), Some(0));
    }

    #[test]
    fn test_select_item() {
        let mut combobox = ComboBox::new(1, 10, 20, 150, 25);
        combobox.add_item(ComboBoxItem::new(1, "Item 1"));
        combobox.add_item(ComboBoxItem::new(2, "Item 2"));

        assert!(combobox.select_item(2));
        assert_eq!(combobox.selected_index(), Some(1));
    }

    #[test]
    fn test_dropdown_toggle() {
        let mut combobox = ComboBox::new(1, 10, 20, 150, 25);
        combobox.add_item(ComboBoxItem::new(1, "Item 1"));

        assert!(!combobox.is_open());
        combobox.toggle();
        assert!(combobox.is_open());
        combobox.toggle();
        assert!(!combobox.is_open());
    }
}
