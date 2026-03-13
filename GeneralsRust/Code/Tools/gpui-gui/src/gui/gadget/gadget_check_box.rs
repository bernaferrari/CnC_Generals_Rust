use gpui::{div, prelude::*, px, rgb, AnyElement};

use crate::gui::source_catalog::{GadgetKind, GadgetPort, GuiPortRecord};

pub const RECORD: GuiPortRecord = GuiPortRecord::new(
    "Gadget/GadgetCheckBox.cpp",
    "crate::gui::gadget::gadget_check_box",
    "Gadget Check Box",
    "Ports dual-state check-like button behavior and owner notifications.",
);

pub const PORT: GadgetPort = GadgetPort::new(
    &RECORD,
    "Check Box",
    "Boolean toggle gadget.",
    "Immediate toggle on press.",
    GadgetKind::CheckBox,
);

pub fn render_demo(label: &str, checked: bool) -> AnyElement {
    div()
        .flex()
        .gap_2()
        .items_center()
        .child(
            div()
                .size(px(18.))
                .rounded_sm()
                .border_1()
                .border_color(rgb(0x8dc0ff))
                .bg(rgb(0x162331))
                .child(if checked { "X" } else { "" }),
        )
        .child(label.to_string())
        .into_any_element()
}
