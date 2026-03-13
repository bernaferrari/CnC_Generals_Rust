use gpui::{div, prelude::*, px, rgb, AnyElement};

use crate::gui::source_catalog::{GadgetKind, GadgetPort, GuiPortRecord};

pub const RECORD: GuiPortRecord = GuiPortRecord::new(
    "Gadget/GadgetVerticalSlider.cpp",
    "crate::gui::gadget::gadget_vertical_slider",
    "Gadget Vertical Slider",
    "Ports vertical slider track, thumb state, and scroll-style interaction.",
);

pub const PORT: GadgetPort = GadgetPort::new(
    &RECORD,
    "Vertical Slider",
    "Continuous value slider laid out vertically.",
    "Drag thumb along a vertical rail.",
    GadgetKind::VerticalSlider,
);

pub fn render_demo(value: f32) -> AnyElement {
    let height = 96.0_f32 * value.clamp(0.0, 1.0);
    div()
        .flex()
        .items_end()
        .h(px(96.))
        .w(px(16.))
        .rounded_full()
        .bg(rgb(0x1f2a35))
        .child(
            div()
                .w(px(16.))
                .h(px(height))
                .rounded_full()
                .bg(rgb(0x8dc0ff)),
        )
        .into_any_element()
}
