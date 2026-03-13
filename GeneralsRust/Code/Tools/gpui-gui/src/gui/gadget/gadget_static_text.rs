use gpui::{div, prelude::*, rgb, AnyElement};

use crate::gui::source_catalog::{GadgetKind, GadgetPort, GuiPortRecord};

pub const RECORD: GuiPortRecord = GuiPortRecord::new(
    "Gadget/GadgetStaticText.cpp",
    "crate::gui::gadget::gadget_static_text",
    "Gadget Static Text",
    "Ports multi-line or single-line text labels with layout-aware wrapping.",
);

pub const PORT: GadgetPort = GadgetPort::new(
    &RECORD,
    "Static Text",
    "Read-only text label gadget.",
    "Wrap, align, and tint text content.",
    GadgetKind::StaticText,
);

pub fn render_demo(label: &str, body: &str) -> AnyElement {
    div()
        .flex()
        .flex_col()
        .gap_1()
        .child(label.to_string())
        .child(
            div()
                .text_sm()
                .text_color(rgb(0x8ea2b4))
                .child(body.to_string()),
        )
        .into_any_element()
}
