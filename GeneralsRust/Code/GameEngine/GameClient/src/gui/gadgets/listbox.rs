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
        let text = text.into();
        Self {
            id,
            text: if text.is_empty() {
                " ".to_string()
            } else {
                text
            },
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ListBoxTextAndColor {
    pub text: String,
    pub color: Color,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ListBoxSelection {
    Single(i32),
    Multiple(Vec<i32>),
}

#[derive(Debug, Clone)]
pub struct ListBoxAddEntry {
    pub row: i32,
    pub column: i32,
    pub overwrite: bool,
    pub data: ListBoxItemData,
    pub color: Option<Color>,
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
    last_double_click_index: Option<usize>,
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
    content_top_inset: u32,
    selection_out_cache: Vec<i32>,
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
            last_double_click_index: None,
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
            content_top_inset: 0,
            selection_out_cache: Vec::new(),
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

    pub fn set_content_top_inset(&mut self, inset: u32) {
        self.content_top_inset = inset.min(self.bounds.height);
        self.set_scroll_offset(self.scroll_offset);
    }

    pub fn content_top_inset(&self) -> u32 {
        self.content_top_inset
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

    pub fn selection_mode(&self) -> SelectionMode {
        self.selection_mode
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
                let has_shifted_row = !self.items.is_empty();
                self.selected_indices = self
                    .selected_indices
                    .iter()
                    .filter_map(|idx| idx.checked_sub(1).or_else(|| has_shifted_row.then_some(0)))
                    .collect();
                self.last_selected = self
                    .last_selected
                    .and_then(|idx| idx.checked_sub(1).or_else(|| has_shifted_row.then_some(0)));
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
        self.push_item(ListBoxItem::new(id, text));
    }

    pub fn add_item_with_data(&mut self, id: i32, text: &str, data: Option<ListBoxItemData>) {
        let mut item = ListBoxItem::new(id, text);
        item.data = data;
        self.push_item(item);
    }

    pub fn add_entry(&mut self, entry: ListBoxAddEntry) -> i32 {
        if entry.column < -1 || entry.column >= self.columns as i32 || entry.row < -1 {
            return -1;
        }

        let column = if entry.column == -1 {
            0
        } else {
            entry.column as usize
        };
        let mut row = if entry.row >= self.items.len() as i32 {
            -1
        } else {
            entry.row
        };

        let needs_new_row = row == -1 || !entry.overwrite;
        if needs_new_row && self.max_length > 0 && self.items.len() >= self.max_length {
            if !self.auto_purge {
                return -1;
            }
            if !self.scroll_buffer(1) {
                return -1;
            }
        }

        let inserted = if row == -1 {
            let item = ListBoxItem::new(self.items.len() as i32, "");
            self.push_item(item)
        } else {
            let row_index = row as usize;
            if !entry.overwrite {
                self.insert_blank_row(row_index);
            }
            row_index
        };
        row = inserted as i32;

        self.apply_entry_to_cell(inserted, column, entry.data, entry.color);

        if self.selection_mode == SelectionMode::Multiple {
            self.selected_indices.retain(|&index| index == 0);
            self.last_selected = self.selected_indices.last().copied();
        } else if self.selected_indices.first() == Some(&inserted) {
            self.selected_indices.clear();
            self.last_selected = None;
        }

        self.update_after_entry_added(inserted);
        row
    }

    fn insert_blank_row(&mut self, index: usize) {
        self.items.insert(index, ListBoxItem::new(index as i32, ""));
        for selected in &mut self.selected_indices {
            if *selected >= index {
                *selected += 1;
            }
        }
        if let Some(last) = self.last_selected.as_mut() {
            if *last >= index {
                *last += 1;
            }
        }
        if self.scroll_offset >= index {
            self.scroll_offset += 1;
        }
    }

    fn apply_entry_to_cell(
        &mut self,
        row: usize,
        column: usize,
        data: ListBoxItemData,
        color: Option<Color>,
    ) {
        let Some(item) = self.items.get_mut(row) else {
            return;
        };

        if item.column_data.len() <= column {
            item.column_data
                .resize_with(column + 1, || ListBoxItemData::Integer(0));
            item.column_colors.resize_with(column + 1, || None);
            item.column_user_data.resize_with(column + 1, || None);
        }

        match &data {
            ListBoxItemData::Text(text) if column == 0 => {
                item.text = if text.is_empty() {
                    " ".to_string()
                } else {
                    text.clone()
                };
                item.text_color = color;
            }
            ListBoxItemData::Image { .. } if column == 0 => {
                item.text.clear();
                item.text_color = None;
            }
            _ => {}
        }

        let is_image = matches!(data, ListBoxItemData::Image { .. });
        item.column_data[column] = data;
        item.column_colors[column] =
            color.or_else(|| is_image.then(|| Color::new(255, 255, 255, 255)));
    }

    fn update_after_entry_added(&mut self, row: usize) {
        if self.force_select && self.selected_indices.is_empty() {
            self.selected_indices.push(row);
            self.last_selected = Some(row);
        }
        if self.auto_scroll {
            self.ensure_visible(row);
        }
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

    pub fn set_item_data_at(
        &mut self,
        row: i32,
        column: i32,
        data: Option<ListBoxItemData>,
    ) -> bool {
        if row < 0 || column < 0 {
            return false;
        }
        self.set_item_column_user_data(row as usize, column as usize, data)
    }

    pub fn get_item_data_at(&self, row: i32, column: i32) -> Option<&ListBoxItemData> {
        if row < 0 || column < 0 {
            return None;
        }
        self.get_item_column_user_data(row as usize, column as usize)
    }

    pub fn get_text_and_color(&self, row: i32, column: i32) -> ListBoxTextAndColor {
        let empty = || ListBoxTextAndColor {
            text: String::new(),
            color: Color::new(0, 0, 0, 0),
        };
        if row < 0 || column < 0 || column >= self.columns as i32 {
            return empty();
        }

        let Some(item) = self.items.get(row as usize) else {
            return empty();
        };
        let column = column as usize;
        let color = item
            .column_colors
            .get(column)
            .and_then(|color| *color)
            .or(if column == 0 { item.text_color } else { None })
            .unwrap_or_else(|| Color::new(0, 0, 0, 0));

        if let Some(data) = item.column_data.get(column) {
            if let ListBoxItemData::Text(text) = data {
                return ListBoxTextAndColor {
                    text: text.clone(),
                    color,
                };
            }
            if matches!(data, ListBoxItemData::Image { .. }) || column != 0 {
                return empty();
            }
        }

        if column == 0 {
            return ListBoxTextAndColor {
                text: item.text.clone(),
                color,
            };
        }

        empty()
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

    pub fn scroll_buffer(&mut self, count: usize) -> bool {
        if count == 0 {
            return true;
        }
        if count > self.items.len() {
            return false;
        }

        self.items.drain(0..count);
        let has_shifted_row = !self.items.is_empty();
        self.selected_indices = self
            .selected_indices
            .iter()
            .filter_map(|idx| match self.selection_mode {
                SelectionMode::Single => idx
                    .checked_sub(count)
                    .or_else(|| has_shifted_row.then_some(0)),
                SelectionMode::Multiple => idx.checked_sub(count),
            })
            .collect();
        self.last_selected = self
            .last_selected
            .and_then(|idx| match self.selection_mode {
                SelectionMode::Single => idx
                    .checked_sub(count)
                    .or_else(|| has_shifted_row.then_some(0)),
                SelectionMode::Multiple => idx.checked_sub(count),
            });
        self.scroll_offset = self.scroll_offset.saturating_sub(count);
        let visible = self.visible_rows();
        self.scroll_offset = self
            .scroll_offset
            .min(self.items.len().saturating_sub(visible));
        self.last_right_click = self.last_right_click.and_then(|rc| {
            let shifted = rc.index.checked_sub(count as i32)?;
            Some(ListBoxRightClick {
                index: shifted,
                mouse_x: rc.mouse_x,
                mouse_y: rc.mouse_y,
            })
        });
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
        let Some(rel_y) = self.content_relative_y(y) else {
            return (-1, -1);
        };
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

    pub fn last_double_click_index(&self) -> Option<usize> {
        self.last_double_click_index
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

    pub fn get_selection(&self) -> ListBoxSelection {
        match self.selection_mode {
            SelectionMode::Single => ListBoxSelection::Single(
                self.selected_indices
                    .first()
                    .map(|index| *index as i32)
                    .unwrap_or(-1),
            ),
            SelectionMode::Multiple => ListBoxSelection::Multiple(
                self.selected_indices
                    .iter()
                    .map(|index| *index as i32)
                    .collect(),
            ),
        }
    }

    pub fn cpp_selection_out_value(&mut self) -> usize {
        match self.get_selection() {
            ListBoxSelection::Single(index) => index as isize as usize,
            ListBoxSelection::Multiple(indices) => {
                self.selection_out_cache = indices;
                self.selection_out_cache.push(-1);
                self.selection_out_cache.as_ptr() as usize
            }
        }
    }

    pub fn toggle_multi_selection(&mut self, index: i32) -> bool {
        if self.selection_mode != SelectionMode::Multiple {
            return false;
        }
        if index < 0 {
            self.selected_indices.clear();
            self.last_selected = None;
            return true;
        }

        let index = index as usize;
        if index >= self.items.len() {
            return false;
        }

        if let Some(pos) = self.selected_indices.iter().position(|&i| i == index) {
            self.selected_indices.remove(pos);
            self.last_selected = self.selected_indices.last().copied();
        } else {
            self.selected_indices.push(index);
            self.last_selected = Some(index);
        }
        true
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
        (self.content_height() / self.item_height).max(1) as usize
    }

    fn item_at_position(&self, x: i32, y: i32) -> Option<usize> {
        if !self.bounds.contains_point(x, y) {
            return None;
        }
        let local_y = self.content_relative_y(y)?;
        let row = (local_y as u32 / self.item_height) as usize;
        let index = self.scroll_offset + row;
        if index < self.items.len() {
            Some(index)
        } else {
            None
        }
    }

    fn content_height(&self) -> u32 {
        self.bounds.height.saturating_sub(self.content_top_inset)
    }

    fn content_relative_y(&self, y: i32) -> Option<i32> {
        let local_y = y - self.bounds.y - self.content_top_inset as i32;
        (local_y >= 0).then_some(local_y)
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

        let deselected = self.selection_mode == SelectionMode::Single
            && !self.force_select
            && !modifiers.ctrl
            && !modifiers.shift
            && self.selected_indices.first() == Some(&index);
        if self.selection_mode == SelectionMode::Multiple {
            let _ = self.toggle_multi_selection(index as i32);
            self.ensure_visible(index);
        } else if deselected {
            self.selected_indices.clear();
            self.last_selected = None;
        } else {
            self.select_index(index, modifiers);
        }

        let mut messages = vec![GadgetMessage::Clicked { gadget_id: self.id }];
        if self.is_double_click(index) {
            self.last_double_click_index = Some(index);
            messages.push(GadgetMessage::Custom {
                gadget_id: self.id,
                data: "double_click".to_string(),
            });
        }
        messages.push(GadgetMessage::ValueChanged {
            gadget_id: self.id,
            value: GadgetValue::Integer(if deselected { -1 } else { index as i32 }),
        });

        messages
    }

    fn handle_keyboard_activate(&mut self) -> Vec<GadgetMessage> {
        if self.audio_feedback {
            if let Some(audio) = TheAudio::get() {
                let event = AudioEventRts::new("GUIComboBoxClick");
                audio.add_audio_event(&event);
            }
        }

        self.last_double_click_index = self.selected_indices.first().copied();
        vec![GadgetMessage::Custom {
            gadget_id: self.id,
            data: "double_click".to_string(),
        }]
    }

    fn select_next_matching_initial(&mut self, ch: char) -> bool {
        if self.selection_mode != SelectionMode::Single || self.items.is_empty() {
            return false;
        }

        let start = self
            .selected_indices
            .first()
            .map(|&index| index + 1)
            .unwrap_or(0);
        let needle = ch.to_lowercase().collect::<String>();

        for offset in 0..self.items.len() {
            let index = (start + offset) % self.items.len();
            let Some(first) = self.items[index].text.chars().next() else {
                continue;
            };
            if first.to_lowercase().collect::<String>() == needle {
                return self.select_index(index, KeyModifiers::none());
            }
        }

        false
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
        self.content_top_inset = self.content_top_inset.min(height);
        self.set_scroll_offset(self.scroll_offset);
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
                    return vec![GadgetMessage::Custom {
                        gadget_id: self.id,
                        data: "input_handled".to_string(),
                    }];
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
                        if self.items.is_empty() {
                            return Vec::new();
                        }
                        let next = self
                            .selected_indices
                            .first()
                            .copied()
                            .unwrap_or(0)
                            .saturating_sub(1);
                        self.select_index(next, *modifiers);
                        return vec![GadgetMessage::ValueChanged {
                            gadget_id: self.id,
                            value: GadgetValue::Integer(next as i32),
                        }];
                    }
                    KeyCode::Down => {
                        if self.items.is_empty() {
                            return Vec::new();
                        }
                        let next = self
                            .selected_indices
                            .first()
                            .map(|&i| (i + 1).min(self.items.len().saturating_sub(1)))
                            .unwrap_or(0);
                        self.select_index(next, *modifiers);
                        return vec![GadgetMessage::ValueChanged {
                            gadget_id: self.id,
                            value: GadgetValue::Integer(next as i32),
                        }];
                    }
                    KeyCode::Home => {
                        if !self.items.is_empty() && self.select_index(0, *modifiers) {
                            return vec![GadgetMessage::ValueChanged {
                                gadget_id: self.id,
                                value: GadgetValue::Integer(0),
                            }];
                        }
                    }
                    KeyCode::End => {
                        if !self.items.is_empty() {
                            let last = self.items.len() - 1;
                            if self.select_index(last, *modifiers) {
                                return vec![GadgetMessage::ValueChanged {
                                    gadget_id: self.id,
                                    value: GadgetValue::Integer(last as i32),
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
                                value: GadgetValue::Integer(next as i32),
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
                                value: GadgetValue::Integer(next as i32),
                            }];
                        }
                    }
                    KeyCode::Char(ch) => {
                        self.select_next_matching_initial(*ch);
                    }
                    _ => {}
                }
                Vec::new()
            }
            InputEvent::KeyUp { key, .. } => {
                if !self.focused {
                    return Vec::new();
                }
                match key {
                    KeyCode::Enter | KeyCode::Space => self.handle_keyboard_activate(),
                    _ => Vec::new(),
                }
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

    fn update(&mut self, _delta_time: f32) {
        // Defensive scroll-offset clamping (C++ gadgets are event-driven and have
        // no per-frame update, but the Rust Gadget trait requires one).
        let visible = self.visible_rows();
        if visible == 0 {
            return;
        }
        let max_offset = self.items.len().saturating_sub(visible);
        if self.scroll_offset > max_offset {
            self.scroll_offset = max_offset;
        }
        // Purge stale selected indices after items were removed externally.
        self.selected_indices.retain(|&idx| idx < self.items.len());
        if self.selected_indices.is_empty() {
            self.last_selected = None;
        }
        if let Some(last) = self.last_selected {
            if last >= self.items.len() {
                self.last_selected = self.selected_indices.last().copied();
            }
        }
    }

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

#[cfg(test)]
mod tests {
    use super::*;

    fn click_first_row(listbox: &mut ListBox) -> Vec<GadgetMessage> {
        listbox.handle_input(&InputEvent::MouseUp {
            x: 1,
            y: 1,
            button: MouseButton::Left,
        })
    }

    fn value_changed_integer(messages: &[GadgetMessage]) -> Option<i32> {
        messages.iter().find_map(|message| {
            if let GadgetMessage::ValueChanged {
                value: GadgetValue::Integer(value),
                ..
            } = message
            {
                Some(*value)
            } else {
                None
            }
        })
    }

    #[test]
    fn single_select_click_toggles_existing_selection_when_not_forced() {
        let mut listbox = ListBox::new(7, 0, 0, 100, 40);
        listbox.add_item_with_id(42, "first");

        let first = click_first_row(&mut listbox);
        assert_eq!(listbox.selected_indices(), &[0]);
        assert_eq!(value_changed_integer(&first), Some(0));

        let second = click_first_row(&mut listbox);
        assert!(listbox.selected_indices().is_empty());
        assert_eq!(value_changed_integer(&second), Some(-1));
    }

    #[test]
    fn content_top_inset_keeps_title_area_out_of_row_hit_testing_like_cpp() {
        let mut listbox = ListBox::new(7, 0, 0, 100, 60).with_item_height(10);
        listbox.add_item_with_id(42, "first");
        listbox.add_item_with_id(43, "second");
        listbox.set_content_top_inset(13);

        assert_eq!(listbox.entry_from_xy(1, 12), (-1, -1));
        assert_eq!(listbox.item_at_position(1, 12), None);
        assert_eq!(listbox.entry_from_xy(1, 13), (0, 0));

        let title_click = listbox.handle_input(&InputEvent::MouseUp {
            x: 1,
            y: 12,
            button: MouseButton::Left,
        });
        assert!(title_click.is_empty());
        assert!(listbox.selected_indices().is_empty());

        let content_click = listbox.handle_input(&InputEvent::MouseUp {
            x: 1,
            y: 13,
            button: MouseButton::Left,
        });
        assert_eq!(value_changed_integer(&content_click), Some(0));
        assert_eq!(listbox.selected_indices(), &[0]);
    }

    #[test]
    fn single_select_force_select_keeps_existing_selection_on_click() {
        let mut listbox = ListBox::new(7, 0, 0, 100, 40);
        listbox.set_force_select(true);
        listbox.add_item_with_id(42, "first");

        let first = click_first_row(&mut listbox);
        assert_eq!(listbox.selected_indices(), &[0]);
        assert_eq!(value_changed_integer(&first), Some(0));

        let second = click_first_row(&mut listbox);
        assert_eq!(listbox.selected_indices(), &[0]);
        assert_eq!(value_changed_integer(&second), Some(0));
    }

    #[test]
    fn single_select_char_key_selects_next_matching_initial_like_cpp() {
        let mut listbox = ListBox::new(7, 0, 0, 100, 40);
        listbox.add_item_with_id(10, "Alpha");
        listbox.add_item_with_id(20, "Bravo");
        listbox.add_item_with_id(30, "Beta");
        listbox.add_item_with_id(40, "Charlie");
        listbox.set_focus(true);
        listbox.select_index(1, KeyModifiers::none());

        let messages = listbox.handle_input(&InputEvent::KeyDown {
            key: KeyCode::Char('b'),
            modifiers: KeyModifiers::none(),
        });

        assert!(messages.is_empty());
        assert_eq!(listbox.selected_indices(), &[2]);

        listbox.handle_input(&InputEvent::KeyDown {
            key: KeyCode::Char('B'),
            modifiers: KeyModifiers::none(),
        });
        assert_eq!(listbox.selected_indices(), &[1]);
    }

    #[test]
    fn multi_select_char_key_is_ignored_like_cpp_multi_listbox() {
        let mut listbox =
            ListBox::new(7, 0, 0, 100, 40).with_selection_mode(SelectionMode::Multiple);
        listbox.add_item_with_id(10, "Alpha");
        listbox.add_item_with_id(20, "Bravo");
        listbox.set_focus(true);
        listbox.select_index(0, KeyModifiers::none());

        let messages = listbox.handle_input(&InputEvent::KeyDown {
            key: KeyCode::Char('b'),
            modifiers: KeyModifiers::none(),
        });

        assert!(messages.is_empty());
        assert_eq!(listbox.selected_indices(), &[0]);
    }

    #[test]
    fn multi_select_mouse_click_toggles_row_without_ctrl_like_cpp() {
        let mut listbox =
            ListBox::new(7, 0, 0, 100, 60).with_selection_mode(SelectionMode::Multiple);
        listbox.add_item_with_id(10, "Alpha");
        listbox.add_item_with_id(20, "Bravo");
        listbox.add_item_with_id(30, "Charlie");
        listbox.set_selected_indices(&[0, 2]);

        let remove_first = listbox.handle_input(&InputEvent::MouseUp {
            x: 1,
            y: 1,
            button: MouseButton::Left,
        });
        assert_eq!(value_changed_integer(&remove_first), Some(0));
        assert_eq!(listbox.selected_indices(), &[2]);

        let add_second = listbox.handle_input(&InputEvent::MouseUp {
            x: 1,
            y: 19,
            button: MouseButton::Left,
        });
        assert_eq!(value_changed_integer(&add_second), Some(1));
        assert_eq!(listbox.selected_indices(), &[2, 1]);
    }

    #[test]
    fn right_mouse_down_is_handled_like_cpp_listbox() {
        let mut listbox = ListBox::new(7, 0, 0, 100, 40);
        let messages = listbox.handle_input(&InputEvent::MouseDown {
            x: 1,
            y: 1,
            button: MouseButton::Right,
        });

        assert!(matches!(
            messages.as_slice(),
            [GadgetMessage::Custom { data, .. }] if data == "input_handled"
        ));
    }

    #[test]
    fn id_and_data_add_paths_apply_cpp_add_entry_buffer_rules() {
        let mut listbox = ListBox::new(7, 0, 0, 100, 40);
        listbox.set_max_length(2);
        listbox.set_auto_purge(true);
        listbox.set_force_select(true);

        listbox.add_item_with_id(10, "Alpha");
        listbox.add_item_with_data(20, "Bravo", Some(ListBoxItemData::Integer(2)));
        listbox.add_item_with_id(30, "Charlie");

        assert_eq!(
            listbox
                .items()
                .iter()
                .map(|item| item.id)
                .collect::<Vec<_>>(),
            vec![20, 30]
        );
        assert_eq!(listbox.selected_indices(), &[0]);
        assert!(matches!(
            listbox.get_item_data(0),
            Some(ListBoxItemData::Integer(2))
        ));
    }

    #[test]
    fn empty_text_entries_are_normalized_like_cpp() {
        let mut listbox = ListBox::new(7, 0, 0, 100, 40);

        listbox.add_item("");
        listbox.add_item_with_id(2, "");

        assert_eq!(listbox.items()[0].text, " ");
        assert_eq!(listbox.items()[1].text, " ");
    }

    #[test]
    fn single_select_scroll_buffer_preserves_shifted_first_row_like_cpp() {
        let mut listbox = ListBox::new(7, 0, 0, 100, 40);
        listbox.add_item_with_id(10, "Alpha");
        listbox.add_item_with_id(20, "Bravo");
        listbox.add_item_with_id(30, "Charlie");
        listbox.select_index(0, KeyModifiers::none());

        assert!(listbox.scroll_buffer(1));

        assert_eq!(
            listbox
                .items()
                .iter()
                .map(|item| item.id)
                .collect::<Vec<_>>(),
            vec![20, 30]
        );
        assert_eq!(listbox.selected_indices(), &[0]);
    }

    #[test]
    fn multi_select_scroll_buffer_drops_purged_entries_and_shifts_rest() {
        let mut listbox =
            ListBox::new(7, 0, 0, 100, 40).with_selection_mode(SelectionMode::Multiple);
        listbox.add_item_with_id(10, "Alpha");
        listbox.add_item_with_id(20, "Bravo");
        listbox.add_item_with_id(30, "Charlie");
        listbox.set_selected_indices(&[0, 2]);

        assert!(listbox.scroll_buffer(1));

        assert_eq!(listbox.selected_indices(), &[1]);
    }

    #[test]
    fn toggle_multi_selection_matches_cpp_multi_only_rules() {
        let mut single = ListBox::new(7, 0, 0, 100, 40);
        single.add_item_with_id(10, "Alpha");
        assert!(!single.toggle_multi_selection(0));
        assert!(single.selected_indices().is_empty());

        let mut multi = ListBox::new(8, 0, 0, 100, 40).with_selection_mode(SelectionMode::Multiple);
        multi.add_item_with_id(10, "Alpha");
        multi.add_item_with_id(20, "Bravo");
        multi.add_item_with_id(30, "Charlie");

        assert!(multi.toggle_multi_selection(1));
        assert_eq!(multi.selected_indices(), &[1]);
        assert!(multi.toggle_multi_selection(2));
        assert_eq!(multi.selected_indices(), &[1, 2]);
        assert!(multi.toggle_multi_selection(1));
        assert_eq!(multi.selected_indices(), &[2]);
        assert!(multi.toggle_multi_selection(-1));
        assert!(multi.selected_indices().is_empty());
    }

    #[test]
    fn get_text_and_color_matches_cpp_invalid_and_text_cell_rules() {
        let mut listbox = ListBox::new(7, 0, 0, 100, 40);
        listbox.set_columns(2);
        let row = listbox.add_item_with_color("Alpha", Color::new(0x11, 0x22, 0x33, 0x44));
        let _ = listbox.set_item_column_data(row, 1, ListBoxItemData::Text("Bravo".to_string()));
        let _ = listbox.set_item_column_color(row, 1, Some(Color::new(0x55, 0x66, 0x77, 0x88)));
        let image_row = listbox.add_item_with_data_and_color(2, "Image", None, None);
        let _ = listbox.set_item_column_data(
            image_row,
            0,
            ListBoxItemData::Image {
                name: "icon".to_string(),
                width: 16,
                height: 16,
                text: Some("ignored".to_string()),
            },
        );

        assert_eq!(
            listbox.get_text_and_color(0, 0),
            ListBoxTextAndColor {
                text: "Alpha".to_string(),
                color: Color::new(0x11, 0x22, 0x33, 0x44),
            }
        );
        assert_eq!(
            listbox.get_text_and_color(0, 1),
            ListBoxTextAndColor {
                text: "Bravo".to_string(),
                color: Color::new(0x55, 0x66, 0x77, 0x88),
            }
        );
        assert_eq!(
            listbox.get_text_and_color(image_row as i32, 0),
            ListBoxTextAndColor {
                text: String::new(),
                color: Color::new(0, 0, 0, 0),
            }
        );
        assert_eq!(
            listbox.get_text_and_color(-1, 0),
            ListBoxTextAndColor {
                text: String::new(),
                color: Color::new(0, 0, 0, 0),
            }
        );
        assert_eq!(
            listbox.get_text_and_color(0, 2),
            ListBoxTextAndColor {
                text: String::new(),
                color: Color::new(0, 0, 0, 0),
            }
        );
    }

    #[test]
    fn get_selection_matches_cpp_single_and_multi_return_shapes() {
        let mut single = ListBox::new(7, 0, 0, 100, 40);
        single.add_item_with_id(10, "Alpha");
        single.add_item_with_id(20, "Bravo");

        assert_eq!(single.get_selection(), ListBoxSelection::Single(-1));
        single.select_index(1, KeyModifiers::none());
        assert_eq!(single.get_selection(), ListBoxSelection::Single(1));
        single.set_selected_indices(&[]);
        assert_eq!(single.get_selection(), ListBoxSelection::Single(-1));

        let mut multi = ListBox::new(8, 0, 0, 100, 40).with_selection_mode(SelectionMode::Multiple);
        multi.add_item_with_id(10, "Alpha");
        multi.add_item_with_id(20, "Bravo");
        multi.add_item_with_id(30, "Charlie");

        assert_eq!(
            multi.get_selection(),
            ListBoxSelection::Multiple(Vec::new())
        );
        multi.set_selected_indices(&[0, 2]);
        assert_eq!(
            multi.get_selection(),
            ListBoxSelection::Multiple(vec![0, 2])
        );
    }

    #[test]
    fn item_data_at_matches_cpp_signed_row_column_rules() {
        let mut listbox = ListBox::new(7, 0, 0, 100, 40);
        listbox.set_columns(2);
        listbox.add_item_with_id(10, "Alpha");

        assert!(listbox.set_item_data_at(0, 1, Some(ListBoxItemData::Integer(99))));
        assert!(matches!(
            listbox.get_item_data_at(0, 1),
            Some(ListBoxItemData::Integer(99))
        ));

        assert!(listbox.set_item_data_at(0, 1, None));
        assert!(listbox.get_item_data_at(0, 1).is_none());
        assert!(!listbox.set_item_data_at(-1, 0, Some(ListBoxItemData::Integer(1))));
        assert!(!listbox.set_item_data_at(0, -1, Some(ListBoxItemData::Integer(1))));
        assert!(!listbox.set_item_data_at(1, 0, Some(ListBoxItemData::Integer(1))));
        assert!(listbox.get_item_data_at(-1, 0).is_none());
        assert!(listbox.get_item_data_at(0, -1).is_none());
        assert!(listbox.get_item_data_at(1, 0).is_none());
    }

    #[test]
    fn add_entry_appends_and_writes_columns_like_cpp() {
        let mut listbox = ListBox::new(7, 0, 0, 100, 40);
        listbox.set_columns(2);

        let row = listbox.add_entry(ListBoxAddEntry {
            row: -1,
            column: -1,
            overwrite: true,
            data: ListBoxItemData::Text("Alpha".to_string()),
            color: Some(Color::new(10, 20, 30, 40)),
        });
        assert_eq!(row, 0);

        let same_row = listbox.add_entry(ListBoxAddEntry {
            row: 0,
            column: 1,
            overwrite: true,
            data: ListBoxItemData::Text("Bravo".to_string()),
            color: Some(Color::new(50, 60, 70, 80)),
        });
        assert_eq!(same_row, 0);

        assert_eq!(
            listbox.get_text_and_color(0, 0),
            ListBoxTextAndColor {
                text: "Alpha".to_string(),
                color: Color::new(10, 20, 30, 40),
            }
        );
        assert_eq!(
            listbox.get_text_and_color(0, 1),
            ListBoxTextAndColor {
                text: "Bravo".to_string(),
                color: Color::new(50, 60, 70, 80),
            }
        );
    }

    #[test]
    fn add_entry_insert_and_overwrite_selection_rules_match_cpp() {
        let mut listbox = ListBox::new(7, 0, 0, 100, 60);
        listbox.add_item_with_id(10, "Alpha");
        listbox.add_item_with_id(20, "Bravo");
        listbox.select_index(1, KeyModifiers::none());

        let inserted = listbox.add_entry(ListBoxAddEntry {
            row: 1,
            column: 0,
            overwrite: false,
            data: ListBoxItemData::Text("Inserted".to_string()),
            color: None,
        });
        assert_eq!(inserted, 1);
        assert_eq!(
            listbox
                .items()
                .iter()
                .map(|item| item.text.as_str())
                .collect::<Vec<_>>(),
            vec!["Alpha", "Inserted", "Bravo"]
        );
        assert_eq!(listbox.selected_indices(), &[2]);

        let overwritten = listbox.add_entry(ListBoxAddEntry {
            row: 2,
            column: 0,
            overwrite: true,
            data: ListBoxItemData::Text("Overwritten".to_string()),
            color: None,
        });
        assert_eq!(overwritten, 2);
        assert!(listbox.selected_indices().is_empty());
        assert_eq!(listbox.items()[2].text, "Overwritten");
    }

    #[test]
    fn add_entry_capacity_and_auto_purge_match_cpp_buffer_rules() {
        let mut full = ListBox::new(7, 0, 0, 100, 40);
        full.set_max_length(2);
        full.add_item_with_id(10, "Alpha");
        full.add_item_with_id(20, "Bravo");
        assert_eq!(
            full.add_entry(ListBoxAddEntry {
                row: -1,
                column: 0,
                overwrite: true,
                data: ListBoxItemData::Text("Charlie".to_string()),
                color: None,
            }),
            -1
        );
        assert_eq!(full.items().len(), 2);

        assert_eq!(
            full.add_entry(ListBoxAddEntry {
                row: 1,
                column: 0,
                overwrite: true,
                data: ListBoxItemData::Text("Replaced".to_string()),
                color: None,
            }),
            1
        );
        assert_eq!(
            full.items()
                .iter()
                .map(|item| item.text.as_str())
                .collect::<Vec<_>>(),
            vec!["Alpha", "Replaced"]
        );

        let mut purging = ListBox::new(8, 0, 0, 100, 40);
        purging.set_max_length(2);
        purging.set_auto_purge(true);
        purging.add_item_with_id(10, "Alpha");
        purging.add_item_with_id(20, "Bravo");
        purging.select_index(0, KeyModifiers::none());

        assert_eq!(
            purging.add_entry(ListBoxAddEntry {
                row: -1,
                column: 0,
                overwrite: true,
                data: ListBoxItemData::Text("Charlie".to_string()),
                color: None,
            }),
            1
        );
        assert_eq!(
            purging
                .items()
                .iter()
                .map(|item| item.text.as_str())
                .collect::<Vec<_>>(),
            vec!["Bravo", "Charlie"]
        );
        assert_eq!(purging.selected_indices(), &[0]);

        let mut insert_purging = ListBox::new(9, 0, 0, 100, 40);
        insert_purging.set_max_length(2);
        insert_purging.set_auto_purge(true);
        insert_purging.add_item_with_id(10, "Alpha");
        insert_purging.add_item_with_id(20, "Bravo");

        assert_eq!(
            insert_purging.add_entry(ListBoxAddEntry {
                row: 1,
                column: 0,
                overwrite: false,
                data: ListBoxItemData::Text("Inserted".to_string()),
                color: None,
            }),
            1
        );
        assert_eq!(
            insert_purging
                .items()
                .iter()
                .map(|item| item.text.as_str())
                .collect::<Vec<_>>(),
            vec!["Bravo", "Inserted"]
        );
    }

    #[test]
    fn add_entry_stores_image_entries_like_cpp_image_path() {
        let mut listbox = ListBox::new(7, 0, 0, 100, 40);

        assert_eq!(
            listbox.add_entry(ListBoxAddEntry {
                row: -1,
                column: 0,
                overwrite: true,
                data: ListBoxItemData::Image {
                    name: "icon".to_string(),
                    width: 16,
                    height: 12,
                    text: Some("Icon".to_string()),
                },
                color: None,
            }),
            0
        );

        assert!(matches!(
            listbox.items()[0].column_data.first(),
            Some(ListBoxItemData::Image {
                name,
                width: 16,
                height: 12,
                text: Some(text),
            }) if name == "icon" && text == "Icon"
        ));
        assert_eq!(
            listbox.items()[0].column_colors.first(),
            Some(&Some(Color::new(255, 255, 255, 255)))
        );
        assert_eq!(
            listbox.get_text_and_color(0, 0),
            ListBoxTextAndColor {
                text: String::new(),
                color: Color::new(0, 0, 0, 0),
            }
        );
    }
}
