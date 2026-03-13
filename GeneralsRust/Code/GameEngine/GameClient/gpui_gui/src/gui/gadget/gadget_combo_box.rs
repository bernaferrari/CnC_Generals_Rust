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

pub fn render_demo(selected: &str) -> AnyElement {
    div()
        .px_3()
        .py_2()
        .rounded_md()
        .border_1()
        .border_color(rgb(0x22303f))
        .bg(rgb(0x111922))
        .flex()
        .justify_between()
        .child(selected.to_string())
        .child("v")
        .into_any_element()
}
