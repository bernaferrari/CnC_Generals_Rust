use gpui::{div, prelude::*, rgb, AnyElement};

use crate::gui::source_catalog::{GadgetKind, GadgetPort, GuiPortRecord};

pub const RECORD: GuiPortRecord = GuiPortRecord::new(
    "Gadget/GadgetListBox.cpp",
    "crate::gui::gadget::gadget_list_box",
    "Gadget List Box",
    "Ports scrollable entry presentation, selection, and item data access for list boxes.",
);

pub const PORT: GadgetPort = GadgetPort::new(
    &RECORD,
    "List Box",
    "Scrollable list of maps, saves, or lobby rows.",
    "Select, double-click, and right-click entries.",
    GadgetKind::ListBox,
);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ListBoxAction {
    SelectionChanged(usize),
    DoubleClicked(usize),
    RightClicked(usize),
    Ignored,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ListBoxState {
    pub entries: Vec<String>,
    pub selected_row: Option<usize>,
    pub display_rows: usize,
    pub top_row: usize,
    pub audio_feedback: bool,
    pub last_click_row: Option<usize>,
    pub last_click_time_ms: u32,
}

impl Default for ListBoxState {
    fn default() -> Self {
        Self {
            entries: Vec::new(),
            selected_row: None,
            display_rows: 6,
            top_row: 0,
            audio_feedback: true,
            last_click_row: None,
            last_click_time_ms: 0,
        }
    }
}

impl ListBoxState {
    pub fn click_row(&mut self, row: usize, time_ms: u32) -> ListBoxAction {
        if row >= self.entries.len() {
            return ListBoxAction::Ignored;
        }

        let is_double = self.last_click_row == Some(row)
            && time_ms.saturating_sub(self.last_click_time_ms) <= 500;
        self.selected_row = Some(row);
        self.last_click_row = Some(row);
        self.last_click_time_ms = time_ms;
        if is_double {
            ListBoxAction::DoubleClicked(row)
        } else {
            ListBoxAction::SelectionChanged(row)
        }
    }

    pub fn right_click_row(&mut self, row: usize) -> ListBoxAction {
        if row >= self.entries.len() {
            return ListBoxAction::Ignored;
        }
        self.selected_row = Some(row);
        ListBoxAction::RightClicked(row)
    }

    pub fn visible_entries(&self) -> &[String] {
        let end = (self.top_row + self.display_rows).min(self.entries.len());
        &self.entries[self.top_row..end]
    }
}

pub fn render_demo(entries: &[&str], selected: &str) -> AnyElement {
    let state = ListBoxState {
        entries: entries.iter().map(|entry| (*entry).to_string()).collect(),
        selected_row: entries.iter().position(|entry| *entry == selected),
        ..Default::default()
    };
    div()
        .flex()
        .flex_col()
        .gap_1()
        .children(state.visible_entries().iter().map(|label| {
            div()
                .px_2()
                .py_1()
                .rounded_sm()
                .bg(if label == selected {
                    rgb(0x223347)
                } else {
                    rgb(0x101720)
                })
                .child(label.clone())
        }))
        .into_any_element()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn repeated_click_within_double_click_window_opens_entry() {
        let mut state = ListBoxState {
            entries: vec!["A".to_string(), "B".to_string()],
            ..Default::default()
        };
        assert_eq!(state.click_row(1, 100), ListBoxAction::SelectionChanged(1));
        assert_eq!(state.click_row(1, 400), ListBoxAction::DoubleClicked(1));
    }
}
