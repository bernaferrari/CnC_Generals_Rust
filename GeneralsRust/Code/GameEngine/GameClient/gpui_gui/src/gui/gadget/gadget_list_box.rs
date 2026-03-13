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

pub fn render_demo(entries: &[&str], selected: &str) -> AnyElement {
    div()
        .flex()
        .flex_col()
        .gap_1()
        .children(entries.iter().map(|label| {
            div()
                .px_2()
                .py_1()
                .rounded_sm()
                .bg(if *label == selected {
                    rgb(0x223347)
                } else {
                    rgb(0x101720)
                })
                .child((*label).to_string())
        }))
        .into_any_element()
}
