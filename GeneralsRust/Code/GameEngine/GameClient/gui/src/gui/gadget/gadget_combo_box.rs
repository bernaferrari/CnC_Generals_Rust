use gpui::{div, prelude::*, rgb, AnyElement};

use crate::gui::source_catalog::{GadgetKind, GadgetPort, GuiPortRecord};

pub const RECORD: GuiPortRecord = GuiPortRecord::new(
    "Gadget/GadgetComboBox.cpp",
    "crate::gui::gadget::gadget_combo_box",
    "Gadget Combo Box",
    "Ports text-plus-dropdown selection and owner messaging for combo boxes.",
);

pub const PORT: GadgetPort = GadgetPort::new(
    &RECORD,
    "Combo Box",
    "Dropdown list with optional editable text.",
    "Expand, choose, and update display text.",
    GadgetKind::ComboBox,
);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ComboBoxAction {
    Expand,
    Collapse,
    SelectionChanged,
    ForwardToEdit,
    NextTab,
    PrevTab,
    Ignored,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ComboBoxState {
    pub selected_text: String,
    pub entries: Vec<String>,
    pub expanded: bool,
    pub max_display: usize,
    pub allow_edit: bool,
    pub dont_hide: bool,
}

impl Default for ComboBoxState {
    fn default() -> Self {
        Self {
            selected_text: String::new(),
            entries: Vec::new(),
            expanded: false,
            max_display: 6,
            allow_edit: true,
            dont_hide: false,
        }
    }
}

impl ComboBoxState {
    pub fn toggle_dropdown(&mut self) -> ComboBoxAction {
        self.dont_hide = false;
        self.expanded = !self.expanded;
        if self.expanded {
            ComboBoxAction::Expand
        } else {
            ComboBoxAction::Collapse
        }
    }

    pub fn hide_list_box(&mut self) -> ComboBoxAction {
        if !self.expanded {
            return ComboBoxAction::Ignored;
        }
        self.expanded = false;
        ComboBoxAction::Collapse
    }

    pub fn visible_entry_count(&self) -> usize {
        self.entries.len().min(self.max_display)
    }

    pub fn select_entry(&mut self, index: usize) -> ComboBoxAction {
        let Some(entry) = self.entries.get(index) else {
            return ComboBoxAction::Ignored;
        };
        self.selected_text = entry.clone();
        self.expanded = false;
        ComboBoxAction::SelectionChanged
    }

    pub fn key_press(&mut self, key: &str, down: bool) -> ComboBoxAction {
        if !down {
            return ComboBoxAction::Ignored;
        }

        match key {
            "Down" | "Right" | "Tab" => ComboBoxAction::NextTab,
            "Up" | "Left" => ComboBoxAction::PrevTab,
            _ if self.allow_edit => ComboBoxAction::ForwardToEdit,
            _ => ComboBoxAction::Ignored,
        }
    }
}

pub fn render_demo(selected: &str) -> AnyElement {
    render(&ComboBoxState {
        selected_text: selected.to_string(),
        ..Default::default()
    })
}

pub fn render(state: &ComboBoxState) -> AnyElement {
    let selected = if state.selected_text.is_empty() {
        "<none>".to_string()
    } else {
        state.selected_text.clone()
    };

    let mut sections = vec![div()
        .px_3()
        .py_2()
        .rounded_md()
        .border_1()
        .border_color(rgb(0x22303f))
        .bg(rgb(0x111922))
        .flex()
        .justify_between()
        .child(selected)
        .child(if state.expanded { "^" } else { "v" })
        .into_any_element()];

    if state.expanded {
        sections.push(
            div()
                .flex()
                .flex_col()
                .gap_1()
                .rounded_md()
                .border_1()
                .border_color(rgb(0x22303f))
                .bg(rgb(0x0e1620))
                .children(
                    state
                        .entries
                        .iter()
                        .take(state.visible_entry_count())
                        .map(|entry| {
                            div()
                                .px_3()
                                .py_1()
                                .text_sm()
                                .text_color(if *entry == state.selected_text {
                                    rgb(0xd6b179)
                                } else {
                                    rgb(0x8ea2b4)
                                })
                                .child(entry.clone())
                        }),
                )
                .into_any_element(),
        );
    }

    div()
        .flex()
        .flex_col()
        .gap_1()
        .children(sections)
        .into_any_element()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dropdown_toggle_and_selection_behave_like_legacy_shell() {
        let mut state = ComboBoxState {
            entries: vec!["Alpha".to_string(), "Beta".to_string()],
            ..Default::default()
        };
        assert_eq!(state.toggle_dropdown(), ComboBoxAction::Expand);
        assert!(state.expanded);
        assert_eq!(state.select_entry(1), ComboBoxAction::SelectionChanged);
        assert_eq!(state.selected_text, "Beta");
        assert!(!state.expanded);
    }

    #[test]
    fn expanded_combobox_limits_visible_entries() {
        let state = ComboBoxState {
            selected_text: "A".to_string(),
            entries: vec![
                "A".to_string(),
                "B".to_string(),
                "C".to_string(),
                "D".to_string(),
                "E".to_string(),
                "F".to_string(),
                "G".to_string(),
            ],
            expanded: true,
            max_display: 4,
            ..Default::default()
        };
        assert_eq!(state.visible_entry_count(), 4);
        let _ = render(&state);
    }
}
