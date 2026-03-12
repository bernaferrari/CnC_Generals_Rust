//! ListBox UI Gadget
//!
//! Scrollable list control with single or multi-selection support.

use super::*;
use crate::gui::Color;
use gamelogic::common::audio::AudioEventRts;
use gamelogic::helpers::TheAudio;
use std::time::{Duration, Instant};

/// List box item
#[derive(Debug, Clone)]
pub struct ListBoxItem {
    pub id: i32,
    pub text: String,
    pub enabled: bool,
    pub text_color: Option<Color>,
    pub data: Option<ListBoxItemData>,
    pub column_data: Vec<ListBoxItemData>,
    pub column_colors: Vec<Option<Color>>,
    pub column_user_data: Vec<Option<ListBoxItemData>>,
}

impl ListBoxItem {
    pub fn new(id: i32, text: impl Into<String>) -> Self {
        Self {
            id,
            text: text.into(),
            enabled: true,
            text_color: None,
            data: None,
            column_data: Vec::new(),
            column_colors: Vec::new(),
            column_user_data: Vec::new(),
        }
    }

    pub fn with_color(mut self, color: Color) -> Self {
        self.text_color = Some(color);
        self
    }
}

/// Optional data stored per list box item.
#[derive(Debug, Clone)]
pub enum ListBoxItemData {
    Integer(i32),
    Text(String),
    Image {
        name: String,
        width: u32,
        height: u32,
        text: Option<String>,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ListBoxRightClick {
    pub index: i32,
    pub mouse_x: i32,
    pub mouse_y: i32,
}

/// Selection mode for list boxes
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SelectionMode {
    Single,
    Multiple,
}

/// List box selection callback
pub type ListBoxCallback = Box<dyn Fn(i32) + Send + Sync>;

/// List box gadget
pub struct ListBox {
    id: GadgetId,
    bounds: Rect,
    content_width: u32,
    state: GadgetState,
    enabled: bool,
    visible: bool,
    focused: bool,
    items: Vec<ListBoxItem>,
    selection_mode: SelectionMode,
    selected_indices: Vec<usize>,
    last_selected: Option<usize>,
    hovered_index: Option<usize>,
    scroll_offset: usize,
    item_height: u32,
    double_click_ms: u64,
    last_click: Option<(Instant, usize)>,
    callback: Option<ListBoxCallback>,
    tooltip: Option<String>,
    max_length: usize,
    auto_purge: bool,
    auto_scroll: bool,
    audio_feedback: bool,
    scroll_if_at_end: bool,
    force_select: bool,
    columns: u32,
    column_width_percentages: Vec<u32>,
    last_right_click: Option<ListBoxRightClick>,
}

impl ListBox {
    pub fn new(id: GadgetId, x: i32, y: i32, width: u32, height: u32) -> Self {
        Self {
            id,
            bounds: Rect::new(x, y, width, height),
            content_width: width,
            state: GadgetState::Normal,
            enabled: true,
            visible: true,
            focused: false,
            items: Vec::new(),
            selection_mode: SelectionMode::Single,
            selected_indices: Vec::new(),
            last_selected: None,
            hovered_index: None,
            scroll_offset: 0,
            item_height: 18,
            double_click_ms: 500,
            last_click: None,
            callback: None,
            tooltip: None,
            max_length: 0,
            auto_purge: false,
            auto_scroll: false,
            audio_feedback: false,
            scroll_if_at_end: false,
            force_select: false,
            columns: 1,
            column_width_percentages: Vec::new(),
            last_right_click: None,
        }
    }

    pub fn with_item_height(mut self, height: u32) -> Self {
        self.item_height = height.max(1);
        self
    }

    pub fn with_selection_mode(mut self, mode: SelectionMode) -> Self {
        self.selection_mode = mode;
        self
    }

    pub fn set_selection_mode(&mut self, mode: SelectionMode) {
        self.selection_mode = mode;
        self.selected_indices.clear();
        self.last_selected = None;
    }

    pub fn set_max_length(&mut self, max_length: usize) {
        self.max_length = max_length;
    }

    pub fn set_auto_purge(&mut self, auto_purge: bool) {
        self.auto_purge = auto_purge;
    }

    pub fn set_auto_scroll(&mut self, auto_scroll: bool) {
        self.auto_scroll = auto_scroll;
    }

    pub fn set_audio_feedback(&mut self, audio_feedback: bool) {
        self.audio_feedback = audio_feedback;
    }

    pub fn set_scroll_if_at_end(&mut self, scroll_if_at_end: bool) {
        self.scroll_if_at_end = scroll_if_at_end;
    }

    pub fn set_force_select(&mut self, force_select: bool) {
        self.force_select = force_select;
    }

    pub fn set_columns(&mut self, columns: u32) {
        self.columns = columns.max(1);
    }

    pub fn columns(&self) -> u32 {
        self.columns
    }

    pub fn set_column_width_percentages(&mut self, widths: Vec<u32>) {
        self.column_width_percentages = widths;
    }

    pub fn set_content_width(&mut self, width: u32) {
        self.content_width = width.max(1);
    }

    pub fn content_width(&self) -> u32 {
        self.content_width.max(1)
    }

    pub fn column_widths_for_width(&self, total_width: u32) -> Vec<u32> {
        let total_width = total_width.max(1);
        if self.columns <= 1 {
            return vec![total_width];
        }

        let columns = self.columns as usize;
        if self.column_width_percentages.len() >= columns {
            return self.column_width_percentages[..columns]
                .iter()
                .map(|pct| total_width.saturating_mul(*pct) / 100)
                .collect();
        }

        let per = total_width / self.columns.max(1);
        vec![per; columns]
    }

    pub fn column_widths(&self) -> Vec<u32> {
        self.column_widths_for_width(self.content_width())
    }

    pub fn items(&self) -> &[ListBoxItem] {
        &self.items
    }

    pub fn selected_indices(&self) -> &[usize] {
        &self.selected_indices
    }

    pub fn scroll_offset(&self) -> usize {
        self.scroll_offset
    }

    pub fn item_height(&self) -> u32 {
        self.item_height
    }

    pub fn with_callback<F>(mut self, callback: F) -> Self
    where
        F: Fn(i32) + Send + Sync + 'static,
    {
        self.callback = Some(Box::new(callback));
        self
    }

    pub fn set_callback<F>(&mut self, callback: F)
    where
        F: Fn(i32) + Send + Sync + 'static,
    {
        self.callback = Some(Box::new(callback));
    }

    pub fn set_tooltip(&mut self, tooltip: impl Into<String>) {
        self.tooltip = Some(tooltip.into());
    }

    pub fn clear(&mut self) {
        self.items.clear();
        self.selected_indices.clear();
        self.last_selected = None;
        self.scroll_offset = 0;
        self.last_right_click = None;
    }

    pub fn add_item(&mut self, text: &str) {
        let item = ListBoxItem::new(self.items.len() as i32, text);
        self.push_item(item);
    }

    pub fn add_item_with_color(&mut self, text: &str, color: Color) -> usize {
        let item = ListBoxItem::new(self.items.len() as i32, text).with_color(color);
        self.push_item(item)
    }

    pub fn add_item_with_data_and_color(
        &mut self,
        id: i32,
        text: &str,
        data: Option<ListBoxItemData>,
        color: Option<Color>,
    ) -> usize {
        let mut item = ListBoxItem::new(id, text);
        item.data = data;
        if let Some(color) = color {
            item.text_color = Some(color);
        }
        self.push_item(item)
    }

    fn push_item(&mut self, item: ListBoxItem) -> usize {
        if self.max_length > 0 && self.items.len() >= self.max_length {
            if self.auto_purge {
                self.items.remove(0);
                self.selected_indices = self
                    .selected_indices
                    .iter()
                    .filter_map(|idx| idx.checked_sub(1))
                    .collect();
                self.last_selected = self.last_selected.and_then(|idx| idx.checked_sub(1));
            } else {
                return self.items.len();
            }
        }
        let visible = self.visible_rows().max(1);
        let was_at_end =
            self.items.len() <= visible || self.scroll_offset + visible >= self.items.len();
        self.items.push(item);
        if self.force_select && self.selected_indices.is_empty() {
            self.selected_indices
                .push(self.items.len().saturating_sub(1));
            self.last_selected = self.selected_indices.first().copied();
        }
        if self.auto_scroll {
            self.scroll_offset = self.items.len().saturating_sub(visible);
        } else if self.scroll_if_at_end && was_at_end && self.items.len() >= visible {
            self.scroll_offset = self.items.len().saturating_sub(visible);
        }
        self.items.len().saturating_sub(1)
    }

    pub fn add_item_with_id(&mut self, id: i32, text: &str) {
        self.items.push(ListBoxItem::new(id, text));
    }

    pub fn add_item_with_data(&mut self, id: i32, text: &str, data: Option<ListBoxItemData>) {
        let mut item = ListBoxItem::new(id, text);
        item.data = data;
        self.items.push(item);
    }

    pub fn set_item_data(&mut self, index: usize, data: Option<ListBoxItemData>) -> bool {
        if let Some(item) = self.items.get_mut(index) {
            item.data = data;
            return true;
        }
        false
    }

    pub fn set_item_column_data(
        &mut self,
        index: usize,
        column: usize,
        data: ListBoxItemData,
    ) -> bool {
        if let Some(item) = self.items.get_mut(index) {
            if item.column_data.len() <= column {
                item.column_data
                    .resize_with(column + 1, || ListBoxItemData::Integer(0));
                item.column_colors.resize_with(column + 1, || None);
                item.column_user_data.resize_with(column + 1, || None);
            }
            if matches!(data, ListBoxItemData::Image { .. }) && item.column_colors[column].is_none()
            {
                item.column_colors[column] = Some(Color::new(255, 255, 255, 255));
            }
            item.column_data[column] = data;
            return true;
        }
        false
    }

    pub fn set_item_column_user_data(
        &mut self,
        index: usize,
        column: usize,
        data: Option<ListBoxItemData>,
    ) -> bool {
        if let Some(item) = self.items.get_mut(index) {
            if item.column_user_data.len() <= column {
                item.column_user_data.resize_with(column + 1, || None);
                if item.column_data.len() <= column {
                    item.column_data
                        .resize_with(column + 1, || ListBoxItemData::Integer(0));
                    item.column_colors.resize_with(column + 1, || None);
                }
            }
            item.column_user_data[column] = data;
            return true;
        }
        false
    }

    pub fn set_item_column_color(
        &mut self,
        index: usize,
        column: usize,
        color: Option<Color>,
    ) -> bool {
        if let Some(item) = self.items.get_mut(index) {
            if item.column_colors.len() <= column {
                item.column_colors.resize_with(column + 1, || None);
                if item.column_data.len() <= column {
                    item.column_data
                        .resize_with(column + 1, || ListBoxItemData::Integer(0));
                    item.column_user_data.resize_with(column + 1, || None);
                }
            }
            item.column_colors[column] = color;
            return true;
        }
        false
    }

    pub fn get_item_column_color(&self, index: usize, column: usize) -> Option<Color> {
        self.items
            .get(index)
            .and_then(|item| item.column_colors.get(column))
            .and_then(|color| *color)
    }

    pub fn set_item_color(&mut self, index: usize, color: Color) -> bool {
        if let Some(item) = self.items.get_mut(index) {
            item.text_color = Some(color);
            return true;
        }
        false
    }

    pub fn get_item_data(&self, index: usize) -> Option<&ListBoxItemData> {
        self.items.get(index).and_then(|item| item.data.as_ref())
    }

    pub fn get_item_column_data(&self, index: usize, column: usize) -> Option<&ListBoxItemData> {
        self.items
            .get(index)
            .and_then(|item| item.column_data.get(column))
    }

    pub fn get_item_column_user_data(
        &self,
        index: usize,
        column: usize,
    ) -> Option<&ListBoxItemData> {
        self.items
            .get(index)
            .and_then(|item| item.column_user_data.get(column))
            .and_then(|data| data.as_ref())
    }

    pub fn remove_item(&mut self, index: usize) -> bool {
        if index >= self.items.len() {
            return false;
        }
        self.items.remove(index);
        self.selected_indices.retain(|&i| i != index);
        self.selected_indices = self
            .selected_indices
            .iter()
            .map(|&i| if i > index { i - 1 } else { i })
            .collect();
        if self.scroll_offset > 0 && self.scroll_offset >= self.items.len() {
            self.scroll_offset = self.items.len().saturating_sub(1);
        }
        if let Some(rc) = self.last_right_click {
            if rc.index == index as i32 {
                self.last_right_click = None;
            } else if rc.index > index as i32 {
                self.last_right_click = Some(ListBoxRightClick {
                    index: rc.index - 1,
                    mouse_x: rc.mouse_x,
                    mouse_y: rc.mouse_y,
                });
            }
        }
        true
    }

    pub fn get_top_visible_entry(&self) -> i32 {
        self.scroll_offset as i32
    }

    pub fn set_top_visible_entry(&mut self, index: i32) {
        let visible = self.visible_rows();
        let max_offset = self.items.len().saturating_sub(visible);
        let clamped = index.clamp(0, max_offset as i32);
        self.scroll_offset = clamped as usize;
    }

    pub fn entry_from_xy(&self, x: i32, y: i32) -> (i32, i32) {
        if !self.bounds.contains_point(x, y) {
            return (-1, -1);
        }
        let rel_y = y - self.bounds.y;
        let row = rel_y / self.item_height as i32;
        let index = self.scroll_offset + row.max(0) as usize;
        if index >= self.items.len() {
            return (-1, -1);
        }
        let col = if self.columns > 1 && self.content_width() > 0 {
            let rel_x = (x - self.bounds.x).max(0) as u32;
            let mut total = 0u32;
            let mut found = -1;
            for (idx, width) in self
                .column_widths_for_width(self.content_width())
                .iter()
                .enumerate()
            {
                total = total.saturating_add(*width);
                if rel_x < total {
                    found = idx as i32;
                    break;
                }
            }
            found
        } else {
            0
        };
        (index as i32, col)
    }

    pub fn last_right_click(&self) -> Option<ListBoxRightClick> {
        self.last_right_click
    }

    pub fn selected_item(&self) -> Option<&ListBoxItem> {
        self.selected_indices
            .first()
            .and_then(|&index| self.items.get(index))
    }

    pub fn set_selected_indices(&mut self, indices: &[usize]) {
        self.selected_indices = indices.iter().cloned().collect();
        self.last_selected = self.selected_indices.last().copied();
    }

    pub fn get_bottom_visible_entry(&self) -> usize {
        let visible = self.visible_rows();
        let bottom = self.scroll_offset + visible;
        bottom.min(self.items.len())
    }

    pub fn select_index(&mut self, index: usize, modifiers: KeyModifiers) -> bool {
        if index >= self.items.len() || !self.items[index].enabled {
            return false;
        }

        match self.selection_mode {
            SelectionMode::Single => {
                self.selected_indices.clear();
                self.selected_indices.push(index);
            }
            SelectionMode::Multiple => {
                if modifiers.ctrl {
                    if let Some(pos) = self.selected_indices.iter().position(|&i| i == index) {
                        self.selected_indices.remove(pos);
                    } else {
                        self.selected_indices.push(index);
                    }
                } else if modifiers.shift {
                    let start = self.last_selected.unwrap_or(index);
                    let (min_i, max_i) = if start <= index {
                        (start, index)
                    } else {
                        (index, start)
                    };
                    self.selected_indices.clear();
                    for i in min_i..=max_i {
                        if self.items[i].enabled {
                            self.selected_indices.push(i);
                        }
                    }
                } else {
                    self.selected_indices.clear();
                    self.selected_indices.push(index);
                }
            }
        }

        self.last_selected = Some(index);
        self.ensure_visible(index);
        self.notify_selection();
        true
    }

    pub fn select_item_id(&mut self, id: i32, modifiers: KeyModifiers) -> bool {
        if let Some(index) = self.items.iter().position(|item| item.id == id) {
            self.select_index(index, modifiers)
        } else {
            false
        }
    }

    pub fn scroll_by(&mut self, delta: i32) {
        let visible = self.visible_rows();
        if visible == 0 {
            return;
        }
        let max_offset = self.items.len().saturating_sub(visible);
        let next = (self.scroll_offset as i32 + delta).clamp(0, max_offset as i32);
        self.scroll_offset = next as usize;
    }

    pub fn set_scroll_offset(&mut self, offset: usize) {
        let visible = self.visible_rows();
        let max_offset = self.items.len().saturating_sub(visible);
        self.scroll_offset = offset.min(max_offset);
    }

    fn visible_rows(&self) -> usize {
        (self.bounds.height / self.item_height).max(1) as usize
    }

    fn item_at_position(&self, x: i32, y: i32) -> Option<usize> {
        if !self.bounds.contains_point(x, y) {
            return None;
        }
        let local_y = y - self.bounds.y;
        let row = (local_y as u32 / self.item_height) as usize;
        let index = self.scroll_offset + row;
        if index < self.items.len() {
            Some(index)
        } else {
            None
        }
    }

    fn ensure_visible(&mut self, index: usize) {
        let visible = self.visible_rows();
        if index < self.scroll_offset {
            self.scroll_offset = index;
        } else if index >= self.scroll_offset + visible {
            self.scroll_offset = index + 1 - visible;
        }
    }

    fn notify_selection(&self) {
        if let Some(item) = self.selected_item() {
            if let Some(callback) = &self.callback {
                callback(item.id);
            }
        }
    }

    fn is_double_click(&mut self, index: usize) -> bool {
        let now = Instant::now();
        let double_click_window = Duration::from_millis(self.double_click_ms);
        let result = if let Some((prev_time, prev_index)) = self.last_click {
            now.duration_since(prev_time) <= double_click_window && prev_index == index
        } else {
            false
        };
        self.last_click = Some((now, index));
        result
    }

    fn handle_click(&mut self, x: i32, y: i32, modifiers: KeyModifiers) -> Vec<GadgetMessage> {
        let Some(index) = self.item_at_position(x, y) else {
            return Vec::new();
        };
        if !self.items[index].enabled {
            return Vec::new();
        }

        if self.audio_feedback {
            if let Some(audio) = TheAudio::get() {
                let event = AudioEventRts::new("GUIComboBoxClick");
                audio.add_audio_event(&event);
            }
        }

        self.select_index(index, modifiers);

        let mut messages = vec![
            GadgetMessage::Clicked { gadget_id: self.id },
            GadgetMessage::ValueChanged {
                gadget_id: self.id,
                value: GadgetValue::Integer(self.items[index].id),
            },
        ];

        if self.is_double_click(index) {
            messages.push(GadgetMessage::Custom {
                gadget_id: self.id,
                data: "double_click".to_string(),
            });
        }

        messages
    }
}

impl Gadget for ListBox {
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
        self.content_width = width.max(1);
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
        self.enabled && self.visible
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
            InputEvent::MouseMove { x, y } => {
                self.hovered_index = self.item_at_position(*x, *y);
                Vec::new()
            }
            InputEvent::MouseDown { x, y, button } => {
                if *button == MouseButton::Left && self.bounds.contains_point(*x, *y) {
                    self.state = GadgetState::Pressed;
                    if self.audio_feedback {
                        if let Some(audio) = TheAudio::get() {
                            let event = AudioEventRts::new("GUIComboBoxClick");
                            audio.add_audio_event(&event);
                        }
                    }
                }
                if *button == MouseButton::Right {
                    if self.audio_feedback {
                        if let Some(audio) = TheAudio::get() {
                            let event = AudioEventRts::new("GUIComboBoxClick");
                            audio.add_audio_event(&event);
                        }
                    }
                }
                Vec::new()
            }
            InputEvent::MouseUp { x, y, button } => {
                self.state = GadgetState::Normal;
                if *button == MouseButton::Left {
                    self.handle_click(*x, *y, KeyModifiers::none())
                } else if *button == MouseButton::Right {
                    let index = self
                        .item_at_position(*x, *y)
                        .map(|idx| idx as i32)
                        .unwrap_or(-1);
                    self.last_right_click = Some(ListBoxRightClick {
                        index,
                        mouse_x: *x,
                        mouse_y: *y,
                    });
                    vec![GadgetMessage::RightClicked { gadget_id: self.id }]
                } else {
                    Vec::new()
                }
            }
            InputEvent::KeyDown { key, modifiers } => {
                if !self.focused {
                    return Vec::new();
                }
                match key {
                    KeyCode::Tab if modifiers.shift == false => {
                        return vec![GadgetMessage::Custom {
                            gadget_id: self.id,
                            data: "tab_next".to_string(),
                        }];
                    }
                    KeyCode::Tab if modifiers.shift => {
                        return vec![GadgetMessage::Custom {
                            gadget_id: self.id,
                            data: "tab_prev".to_string(),
                        }];
                    }
                    KeyCode::Up => {
                        let next = self
                            .selected_indices
                            .first()
                            .copied()
                            .unwrap_or(0)
                            .saturating_sub(1);
                        if self.select_index(next, *modifiers) {
                            return vec![GadgetMessage::ValueChanged {
                                gadget_id: self.id,
                                value: GadgetValue::Integer(self.items[next].id),
                            }];
                        }
                    }
                    KeyCode::Down => {
                        let next = self
                            .selected_indices
                            .first()
                            .map(|&i| (i + 1).min(self.items.len().saturating_sub(1)))
                            .unwrap_or(0);
                        if self.select_index(next, *modifiers) {
                            return vec![GadgetMessage::ValueChanged {
                                gadget_id: self.id,
                                value: GadgetValue::Integer(self.items[next].id),
                            }];
                        }
                    }
                    KeyCode::Home => {
                        if !self.items.is_empty() && self.select_index(0, *modifiers) {
                            return vec![GadgetMessage::ValueChanged {
                                gadget_id: self.id,
                                value: GadgetValue::Integer(self.items[0].id),
                            }];
                        }
                    }
                    KeyCode::End => {
                        if !self.items.is_empty() {
                            let last = self.items.len() - 1;
                            if self.select_index(last, *modifiers) {
                                return vec![GadgetMessage::ValueChanged {
                                    gadget_id: self.id,
                                    value: GadgetValue::Integer(self.items[last].id),
                                }];
                            }
                        }
                    }

                    KeyCode::PageUp => {
                        let visible = self.visible_rows();
                        let current = self.selected_indices.first().copied().unwrap_or(0);
                        let next = current.saturating_sub(visible);
                        if self.select_index(next, *modifiers) {
                            return vec![GadgetMessage::ValueChanged {
                                gadget_id: self.id,
                                value: GadgetValue::Integer(self.items[next].id),
                            }];
                        }
                    }
                    KeyCode::PageDown => {
                        let visible = self.visible_rows();
                        let current = self.selected_indices.first().copied().unwrap_or(0);
                        let next = (current + visible).min(self.items.len().saturating_sub(1));
                        if self.select_index(next, *modifiers) {
                            return vec![GadgetMessage::ValueChanged {
                                gadget_id: self.id,
                                value: GadgetValue::Integer(self.items[next].id),
                            }];
                        }
                    }
                    _ => {}
                }
                Vec::new()
            }
            InputEvent::FocusGained => {
                self.set_focus(true);
                vec![GadgetMessage::FocusChanged {
                    gadget_id: self.id,
                    has_focus: true,
                }]
            }
            InputEvent::FocusLost => {
                self.set_focus(false);
                vec![GadgetMessage::FocusChanged {
                    gadget_id: self.id,
                    has_focus: false,
                }]
            }
            InputEvent::MouseEnter { .. } => {
                if self.enabled {
                    self.state = GadgetState::Hovered;
                }
                vec![GadgetMessage::MouseEnter { gadget_id: self.id }]
            }
            InputEvent::MouseLeave { .. } => {
                self.hovered_index = None;
                if self.enabled {
                    self.state = if self.focused {
                        GadgetState::Hovered
                    } else {
                        GadgetState::Normal
                    };
                }
                vec![GadgetMessage::MouseLeave { gadget_id: self.id }]
            }
            InputEvent::MouseDrag { button, .. } => {
                if *button == MouseButton::Left {
                    vec![GadgetMessage::LeftDrag { gadget_id: self.id }]
                } else {
                    Vec::new()
                }
            }
            _ => Vec::new(),
        }
    }

    fn update(&mut self, _delta_time: f32) {}

    fn render(&self, _theme: &GadgetTheme) {
        if !self.visible {
            return;
        }
        // Rendering handled by UI layer.
    }

    fn tooltip(&self) -> Option<&str> {
        self.tooltip.as_deref()
    }
}
