use gpui::{div, prelude::*, px, rgb, AnyElement};

use crate::gui::source_catalog::{GadgetKind, GadgetPort, GuiPortRecord};

pub const RECORD: GuiPortRecord = GuiPortRecord::new(
    "Gadget/GadgetProgressBar.cpp",
    "crate::gui::gadget::gadget_progress_bar",
    "Gadget Progress Bar",
    "Ports fill-percentage rendering for build, load, and transfer progress.",
);

pub const PORT: GadgetPort = GadgetPort::new(
    &RECORD,
    "Progress Bar",
    "Non-interactive progress indicator.",
    "Reflects percentage and completion state.",
    GadgetKind::ProgressBar,
);

pub fn render_demo(label: &str, progress: f32) -> AnyElement {
    let width = 144.0_f32 * progress.clamp(0.0, 1.0);
    div()
        .flex()
        .flex_col()
        .gap_1()
        .child(
            div().h(px(10.)).rounded_full().bg(rgb(0x1e2935)).child(
                div()
                    .w(px(width))
                    .h(px(10.))
                    .rounded_full()
                    .bg(rgb(0xd88a44)),
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
