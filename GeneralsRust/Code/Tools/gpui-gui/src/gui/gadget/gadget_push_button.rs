use gpui::{div, prelude::*, rgb, AnyElement};

use crate::gui::source_catalog::{GadgetKind, GadgetPort, GuiPortRecord};

pub const RECORD: GuiPortRecord = GuiPortRecord::new(
    "Gadget/GadgetPushButton.cpp",
    "crate::gui::gadget::gadget_push_button",
    "Gadget Push Button",
    "Ports mouse enter, mouse leave, press, release, and owner message routing for push buttons.",
);

pub const PORT: GadgetPort = GadgetPort::new(
    &RECORD,
    "Push Button",
    "Primary command and menu activation button.",
    "Hover, press, release, optional mouse-down trigger.",
    GadgetKind::PushButton,
);

pub fn render_demo(label: &str) -> AnyElement {
    div()
        .px_4()
        .py_2()
        .rounded_md()
        .border_1()
        .border_color(rgb(0xd1a65d))
        .bg(rgb(0x1f1910))
        .child(label.to_string())
        .into_any_element()
}
