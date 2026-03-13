use gpui::{div, prelude::*, rgb, AnyElement};

use crate::gui::source_catalog::{GadgetKind, GadgetPort, GuiPortRecord};

pub const RECORD: GuiPortRecord = GuiPortRecord::new(
    "Gadget/GadgetTabControl.cpp",
    "crate::gui::gadget::gadget_tab_control",
    "Gadget Tab Control",
    "Ports tab selection and pane switching across grouped window content.",
);

pub const PORT: GadgetPort = GadgetPort::new(
    &RECORD,
    "Tab Control",
    "Tabs that switch visible panes.",
    "Activate tabs and change pane focus.",
    GadgetKind::TabControl,
);

pub fn render_demo(labels: &[&str], active: &str) -> AnyElement {
    div()
        .flex()
        .gap_1()
        .children(labels.iter().map(|label| {
            div()
                .px_3()
                .py_1()
                .rounded_t_md()
                .border_1()
                .border_color(rgb(0x22303f))
                .bg(if *label == active {
                    rgb(0x18232f)
                } else {
                    rgb(0x101720)
                })
                .child((*label).to_string())
        }))
        .into_any_element()
}
