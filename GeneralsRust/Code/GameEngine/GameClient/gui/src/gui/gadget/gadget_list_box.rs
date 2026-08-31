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
    pub focused: bool,
    pub multi_select: bool,
    pub has_up_button: bool,
    pub has_down_button: bool,
    pub double_click_ms: u32,
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
            focused: false,
            multi_select: false,
            has_up_button: false,
            has_down_button: false,
            double_click_ms: os_double_click_time_ms(),
        }
    }
}

impl ListBoxState {
    pub fn click_row(&mut self, row: usize, time_ms: u32) -> ListBoxAction {
        if row >= self.entries.len() {
            return ListBoxAction::Ignored;
        }

        // C++ GadgetListBox LEFT_UP: winSetFocus + GetDoubleClickTime window.
        self.focused = true;
        let is_double = self.last_click_row == Some(row)
            && time_ms.saturating_sub(self.last_click_time_ms) <= self.double_click_ms;
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
        self.focused = true;
        self.selected_row = Some(row);
        ListBoxAction::RightClicked(row)
    }

    pub fn visible_entries(&self) -> &[String] {
        let end = (self.top_row + self.display_rows).min(self.entries.len());
        &self.entries[self.top_row..end]
    }

    /// C++ GadgetListBoxInput ignores Home/End/PgUp/PgDn.
    pub fn key_press(&mut self, key: &str, down: bool) -> ListBoxAction {
        if !down || !self.focused {
            return ListBoxAction::Ignored;
        }
        match key {
            "Home" | "End" | "PgUp" | "PgDn" | "PageUp" | "PageDown" => ListBoxAction::Ignored,
            _ => ListBoxAction::Ignored,
        }
    }

    /// Single-list wheel always scrolls; multi-list only if up/down buttons exist.
    pub fn wheel(&mut self, down: bool) -> ListBoxAction {
        if self.entries.is_empty() {
            return ListBoxAction::Ignored;
        }
        if self.multi_select {
            let has_button = if down {
                self.has_down_button
            } else {
                self.has_up_button
            };
            if !has_button {
                return ListBoxAction::Ignored;
            }
        }
        let max_top = self.entries.len().saturating_sub(self.display_rows.max(1));
        let next = if down {
            (self.top_row + 1).min(max_top)
        } else {
            self.top_row.saturating_sub(1)
        };
        if next == self.top_row {
            ListBoxAction::Ignored
        } else {
            self.top_row = next;
            ListBoxAction::Ignored
        }
    }
}

pub fn render_demo(entries: &[&str], selected: &str) -> AnyElement {
    render(&ListBoxState {
        entries: entries.iter().map(|entry| (*entry).to_string()).collect(),
        selected_row: entries.iter().position(|entry| *entry == selected),
        ..Default::default()
    })
}

pub fn render(state: &ListBoxState) -> AnyElement {
    div()
        .flex()
        .flex_col()
        .gap_1()
        .children(
            state
                .visible_entries()
                .iter()
                .enumerate()
                .map(|(offset, label)| {
                    let row = state.top_row + offset;
                    div()
                        .px_2()
                        .py_1()
                        .rounded_sm()
                        .bg(if state.selected_row == Some(row) {
                            rgb(0x223347)
                        } else {
                            rgb(0x101720)
                        })
                        .child(label.clone())
                }),
        )
        .into_any_element()
}


/// C++ `GetDoubleClickTime()` (`GadgetListBox.cpp:43`).
pub fn os_double_click_time_ms() -> u32 {
    #[cfg(windows)]
    {
        #[link(name = "user32")]
        // SAFETY: Declaration of the documented Win32 GetDoubleClickTime symbol in
        // SAFETY: user32; signature matches the Windows SDK and it has no safety
        // SAFETY: preconditions beyond running on Windows.
        unsafe extern "system" {
            fn GetDoubleClickTime() -> u32;
        }
        // SAFETY: user32 is linked above and GetDoubleClickTime is a parameterless
        // SAFETY: thread-safe query returning a millisecond count; no pointers.
        return unsafe { GetDoubleClickTime() }.max(1);
    }
    #[cfg(not(windows))]
    {
        500
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn repeated_click_within_os_double_click_window_opens_entry() {
        let mut state = ListBoxState {
            entries: vec!["A".to_string(), "B".to_string()],
            ..Default::default()
        };
        let window = state.double_click_ms;
        assert_eq!(state.click_row(1, 0), ListBoxAction::SelectionChanged(1));
        assert!(state.focused);
        assert_eq!(
            state.click_row(1, window.saturating_sub(1)),
            ListBoxAction::DoubleClicked(1)
        );
    }

    #[test]
    fn home_end_page_keys_are_ignored_like_cpp() {
        let mut state = ListBoxState {
            entries: vec!["A".to_string(), "B".to_string()],
            focused: true,
            selected_row: Some(1),
            ..Default::default()
        };
        for key in ["Home", "End", "PgUp", "PgDn"] {
            assert_eq!(state.key_press(key, true), ListBoxAction::Ignored);
            assert_eq!(state.selected_row, Some(1));
        }
    }

    #[test]
    fn multi_list_wheel_requires_buttons_like_cpp() {
        let mut state = ListBoxState {
            entries: (0..10).map(|i| i.to_string()).collect(),
            display_rows: 3,
            multi_select: true,
            ..Default::default()
        };
        assert_eq!(state.wheel(true), ListBoxAction::Ignored);
        assert_eq!(state.top_row, 0);
        state.has_down_button = true;
        assert_eq!(state.wheel(true), ListBoxAction::Ignored);
        assert_eq!(state.top_row, 1);
    }
}
