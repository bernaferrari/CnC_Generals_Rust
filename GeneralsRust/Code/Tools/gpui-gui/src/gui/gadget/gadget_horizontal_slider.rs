use gpui::{div, prelude::*, px, rgb, AnyElement};

use crate::gui::source_catalog::{GadgetKind, GadgetPort, GuiPortRecord};

pub const RECORD: GuiPortRecord = GuiPortRecord::new(
    "Gadget/GadgetHorizontalSlider.cpp",
    "crate::gui::gadget::gadget_horizontal_slider",
    "Gadget Horizontal Slider",
    "Ports horizontal slider track, thumb hover, and drag-to-value behavior.",
);

pub const PORT: GadgetPort = GadgetPort::new(
    &RECORD,
    "Horizontal Slider",
    "Continuous value slider laid out horizontally.",
    "Drag thumb along a horizontal rail.",
    GadgetKind::HorizontalSlider,
);

pub fn render_demo(label: &str, value: f32) -> AnyElement {
    let width = 144.0_f32 * value.clamp(0.0, 1.0);
    div()
        .flex()
        .flex_col()
        .gap_1()
        .child(
            div().h(px(8.)).rounded_full().bg(rgb(0x1f2a35)).child(
                div()
                    .w(px(width))
                    .h(px(8.))
                    .rounded_full()
                    .bg(rgb(0x69d18a)),
            ),
        )
        .child(
            div()
                .text_sm()
                .text_color(rgb(0x8ea2b4))
                .child(label.to_string()),
        )
        .into_any_element()
}
