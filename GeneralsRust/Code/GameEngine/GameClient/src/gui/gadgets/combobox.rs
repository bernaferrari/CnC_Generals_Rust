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
    pub data: Option<usize>,
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
        self.data = Some(data as usize);
        self
    }
}

/// ComboBox selection callback
pub type ComboBoxCallback = Box<dyn Fn(u32) + Send + Sync>;

/// Draw command emitted by [`ComboBox`] for the UI renderer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ComboBoxRenderCommand {
    MainBox {
        rect: Rect,
        color: Color,
        border_color: Color,
        border_width: u32,
    },
    MainText {
        rect: Rect,
        text: String,
        color: Color,
    },
    DropDownButton {
        rect: Rect,
        color: Color,
        border_color: Color,
        border_width: u32,
    },
    DropDownArrow {
        rect: Rect,
        color: Color,
    },
    DropDownBackground {
        rect: Rect,
        color: Color,
        border_color: Color,
        border_width: u32,
    },
    Item {
        rect: Rect,
        index: usize,
        text: String,
        color: Color,
        text_color: Color,
        selected: bool,
        hovered: bool,
        enabled: bool,
    },
}

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
    }

    pub fn set_item_data(&mut self, index: usize, data: i32) -> bool {
        self.set_item_data_raw(index, data as usize)
    }

    pub fn set_item_data_raw(&mut self, index: usize, data: usize) -> bool {
        if let Some(item) = self.items.get_mut(index) {
            item.data = Some(data);
            return true;
        }
        false
    }

    pub fn selected_item_data(&self) -> Option<i32> {
        self.selected_item_data_raw().map(|data| data as i32)
    }

    pub fn selected_item_data_raw(&self) -> Option<usize> {
        self.selected_index
            .and_then(|index| self.items.get(index))
            .and_then(|item| item.data)
    }

    pub fn item_data_raw(&self, index: usize) -> Option<usize> {
        self.items.get(index).and_then(|item| item.data)
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

    pub fn clear_selection(&mut self) {
        self.selected_index = None;
    }

    pub fn set_text(&mut self, text: impl Into<String>) {
        self.text = text.into();
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

    pub fn is_editable(&self) -> bool {
        self.is_editable
    }

    pub fn set_max_chars(&mut self, max_chars: usize) {
        self.max_chars = max_chars;
    }

    pub fn max_chars(&self) -> usize {
        self.max_chars
    }

    pub fn set_ascii_only(&mut self, ascii_only: bool) {
        self.ascii_only = ascii_only;
    }

    pub fn ascii_only(&self) -> bool {
        self.ascii_only
    }

    pub fn set_letters_and_numbers(&mut self, letters_only: bool) {
        self.letters_and_numbers = letters_only;
    }

    pub fn letters_and_numbers(&self) -> bool {
        self.letters_and_numbers
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

    fn drop_down_button_width(&self) -> u32 {
        self.bounds.width.min(21)
    }

    fn edit_box_rect(&self) -> Rect {
        let button_width = self.drop_down_button_width();
        Rect::new(
            self.bounds.x,
            self.bounds.y,
            self.bounds.width.saturating_sub(button_width),
            self.bounds.height,
        )
    }

    fn drop_down_button_rect(&self) -> Rect {
        let button_width = self.drop_down_button_width();
        Rect::new(
            self.bounds.x + self.bounds.width.saturating_sub(button_width) as i32,
            self.bounds.y,
            button_width,
            self.bounds.height,
        )
    }

    fn box_color(&self, theme: &GadgetTheme) -> Color {
        if !self.enabled {
            theme.disabled_color
        } else if self.focused {
            theme.focused_color
        } else {
            match self.state {
                GadgetState::Hovered => theme.hovered_color,
                GadgetState::Pressed => theme.pressed_color,
                GadgetState::Disabled => theme.disabled_color,
                GadgetState::Focused => theme.focused_color,
                GadgetState::Normal => theme.normal_color,
            }
        }
    }

    fn item_color(&self, theme: &GadgetTheme, index: usize, item: &ComboBoxItem) -> Color {
        if !item.enabled {
            theme.disabled_color
        } else if self.selected_index == Some(index) {
            theme.pressed_color
        } else if self.hovered_item == Some(index) {
            theme.hovered_color
        } else {
            theme.normal_color
        }
    }

    /// Build renderer-facing commands for the current combobox state.
    pub fn render_commands(&self, theme: &GadgetTheme) -> Vec<ComboBoxRenderCommand> {
        if !self.visible {
            return Vec::new();
        }

        let box_color = self.box_color(theme);
        let text_color = if self.enabled {
            theme.text_color
        } else {
            theme.disabled_text_color
        };
        let edit_rect = self.edit_box_rect();
        let button_rect = self.drop_down_button_rect();

        let mut commands = vec![
            ComboBoxRenderCommand::MainBox {
                rect: edit_rect,
                color: box_color,
                border_color: theme.border_color,
                border_width: theme.border_width,
            },
            ComboBoxRenderCommand::MainText {
                rect: edit_rect,
                text: self.text().to_string(),
                color: text_color,
            },
            ComboBoxRenderCommand::DropDownButton {
                rect: button_rect,
                color: box_color,
                border_color: theme.border_color,
                border_width: theme.border_width,
            },
            ComboBoxRenderCommand::DropDownArrow {
                rect: button_rect,
                color: text_color,
            },
        ];

        if self.dropdown_open {
            let dropdown = self.dropdown_bounds_internal();
            commands.push(ComboBoxRenderCommand::DropDownBackground {
                rect: dropdown,
                color: theme.normal_color,
                border_color: theme.border_color,
                border_width: theme.border_width,
            });

            let visible_count = if self.item_height == 0 {
                0
            } else {
                (dropdown.height / self.item_height) as usize
            };

            for (index, item) in self.items.iter().take(visible_count).enumerate() {
                let item_rect = Rect::new(
                    dropdown.x,
                    dropdown.y + (index as u32 * self.item_height) as i32,
                    dropdown.width,
                    self.item_height,
                );
                let item_text_color = if item.enabled {
                    theme.text_color
                } else {
                    theme.disabled_text_color
                };
                commands.push(ComboBoxRenderCommand::Item {
                    rect: item_rect,
                    index,
                    text: item.text.clone(),
                    color: self.item_color(theme, index, item),
                    text_color: item_text_color,
                    selected: self.selected_index == Some(index),
                    hovered: self.hovered_item == Some(index),
                    enabled: item.enabled,
                });
            }
        }

        commands
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
                if *button == MouseButton::Left
                    && self.dropdown_open
                    && !self.bounds.contains_point(*x, *y)
                {
                    let dropdown = self.dropdown_bounds_internal();
                    if !dropdown.contains_point(*x, *y) {
                        self.close();
                    }
                }
            }

            InputEvent::MouseUp { x, y, button } => {
                if *button == MouseButton::Left {
                    if self.bounds.contains_point(*x, *y) {
                        self.toggle();
                    } else if self.dropdown_open {
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
            }

            InputEvent::KeyDown { key, .. } if self.focused => match key {
                KeyCode::Enter | KeyCode::Space => {
                    self.toggle();
                }
                KeyCode::Escape => {
                    self.close();
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
                KeyCode::Backspace => {
                    if self.is_editable && !self.text.is_empty() {
                        self.text.pop();
                        return vec![GadgetMessage::ValueChanged {
                            gadget_id: self.id,
                            value: GadgetValue::String(self.text.clone()),
                        }];
                    }
                }
                KeyCode::Char(ch) if self.is_editable => {
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
                _ => {}
            },

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

        let _commands = self.render_commands(theme);
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
        assert_eq!(combobox.selected_index(), None);
        assert_eq!(combobox.text(), "");
        assert!(!combobox.is_open());
    }

    #[test]
    fn select_index_sets_text_after_explicit_selection_like_cpp() {
        let mut combobox = ComboBox::new(1, 10, 20, 150, 25);
        combobox.add_item(ComboBoxItem::new(1, "Item 1"));
        combobox.add_item(ComboBoxItem::new(2, "Item 2"));

        assert!(combobox.select_index(0));
        assert_eq!(combobox.selected_index(), Some(0));
        assert_eq!(combobox.text(), "Item 1");
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

    #[test]
    fn keyboard_arrows_and_tab_request_focus_navigation_like_cpp() {
        let mut combobox = ComboBox::new(1, 10, 20, 150, 25);
        combobox.add_item(ComboBoxItem::new(1, "Item 1"));
        combobox.add_item(ComboBoxItem::new(2, "Item 2"));
        combobox.select_index(0);
        combobox.set_focus(true);

        let next = combobox.handle_input(&InputEvent::KeyDown {
            key: KeyCode::Down,
            modifiers: KeyModifiers::none(),
        });
        assert_eq!(combobox.selected_index(), Some(0));
        assert!(matches!(
            next.as_slice(),
            [GadgetMessage::Custom { gadget_id: 1, data } ] if data == "tab_next"
        ));

        let prev = combobox.handle_input(&InputEvent::KeyDown {
            key: KeyCode::Up,
            modifiers: KeyModifiers::none(),
        });
        assert_eq!(combobox.selected_index(), Some(0));
        assert!(matches!(
            prev.as_slice(),
            [GadgetMessage::Custom { gadget_id: 1, data } ] if data == "tab_prev"
        ));
    }

    #[test]
    fn opens_on_left_up_not_left_down_like_cpp() {
        let mut combobox = ComboBox::new(1, 10, 20, 150, 25);
        combobox.add_item(ComboBoxItem::new(1, "Item 1"));

        let down = combobox.handle_input(&InputEvent::MouseDown {
            x: 20,
            y: 25,
            button: MouseButton::Left,
        });
        assert!(down.is_empty());
        assert!(!combobox.is_open());

        let up = combobox.handle_input(&InputEvent::MouseUp {
            x: 20,
            y: 25,
            button: MouseButton::Left,
        });
        assert!(up.is_empty());
        assert!(combobox.is_open());

        let up = combobox.handle_input(&InputEvent::MouseUp {
            x: 20,
            y: 25,
            button: MouseButton::Left,
        });
        assert!(up.is_empty());
        assert!(!combobox.is_open());
    }

    #[test]
    fn combobox_render_commands_cover_main_edit_and_dropdown_button() {
        let theme = GadgetTheme::default();
        let mut combobox = ComboBox::new(1, 10, 20, 150, 25);
        combobox.add_item(ComboBoxItem::new(1, "Item 1"));
        combobox.select_index(0);

        assert_eq!(
            combobox.render_commands(&theme),
            vec![
                ComboBoxRenderCommand::MainBox {
                    rect: Rect::new(10, 20, 129, 25),
                    color: theme.normal_color,
                    border_color: theme.border_color,
                    border_width: theme.border_width,
                },
                ComboBoxRenderCommand::MainText {
                    rect: Rect::new(10, 20, 129, 25),
                    text: "Item 1".to_string(),
                    color: theme.text_color,
                },
                ComboBoxRenderCommand::DropDownButton {
                    rect: Rect::new(139, 20, 21, 25),
                    color: theme.normal_color,
                    border_color: theme.border_color,
                    border_width: theme.border_width,
                },
                ComboBoxRenderCommand::DropDownArrow {
                    rect: Rect::new(139, 20, 21, 25),
                    color: theme.text_color,
                },
            ]
        );
    }

    #[test]
    fn combobox_render_commands_cover_open_list_visible_items_and_hidden() {
        let theme = GadgetTheme::default();
        let mut combobox = ComboBox::new(1, 10, 20, 150, 25);
        combobox.add_item(ComboBoxItem::new(1, "Item 1"));
        combobox.add_item(ComboBoxItem::new(2, "Item 2"));
        combobox.add_item(ComboBoxItem::new(3, "Item 3"));
        combobox.set_max_display(2);
        combobox.select_index(1);
        combobox.open();

        let messages = combobox.handle_input(&InputEvent::MouseMove { x: 20, y: 72 });
        assert!(messages.is_empty());
        assert_eq!(combobox.hovered_item(), Some(1));

        assert_eq!(
            combobox.render_commands(&theme),
            vec![
                ComboBoxRenderCommand::MainBox {
                    rect: Rect::new(10, 20, 129, 25),
                    color: theme.normal_color,
                    border_color: theme.border_color,
                    border_width: theme.border_width,
                },
                ComboBoxRenderCommand::MainText {
                    rect: Rect::new(10, 20, 129, 25),
                    text: "Item 2".to_string(),
                    color: theme.text_color,
                },
                ComboBoxRenderCommand::DropDownButton {
                    rect: Rect::new(139, 20, 21, 25),
                    color: theme.normal_color,
                    border_color: theme.border_color,
                    border_width: theme.border_width,
                },
                ComboBoxRenderCommand::DropDownArrow {
                    rect: Rect::new(139, 20, 21, 25),
                    color: theme.text_color,
                },
                ComboBoxRenderCommand::DropDownBackground {
                    rect: Rect::new(10, 45, 150, 50),
                    color: theme.normal_color,
                    border_color: theme.border_color,
                    border_width: theme.border_width,
                },
                ComboBoxRenderCommand::Item {
                    rect: Rect::new(10, 45, 150, 25),
                    index: 0,
                    text: "Item 1".to_string(),
                    color: theme.normal_color,
                    text_color: theme.text_color,
                    selected: false,
                    hovered: false,
                    enabled: true,
                },
                ComboBoxRenderCommand::Item {
                    rect: Rect::new(10, 70, 150, 25),
                    index: 1,
                    text: "Item 2".to_string(),
                    color: theme.pressed_color,
                    text_color: theme.text_color,
                    selected: true,
                    hovered: true,
                    enabled: true,
                },
            ]
        );

        combobox.set_visible(false);
        assert!(combobox.render_commands(&theme).is_empty());
    }
}
