use gpui::{div, prelude::*, rgb, AnyElement};

use crate::gui::source_catalog::{GadgetKind, GadgetPort, GuiPortRecord};

pub const RECORD: GuiPortRecord = GuiPortRecord::new(
    "Gadget/GadgetTextEntry.cpp",
    "crate::gui::gadget::gadget_text_entry",
    "Gadget Text Entry",
    "Ports editable text fields, secret text, numeric filtering, and IME-aware input.",
);

pub const PORT: GadgetPort = GadgetPort::new(
    &RECORD,
    "Text Entry",
    "Editable single-line text field.",
    "Keyboard focus, selection, and filtered input.",
    GadgetKind::TextEntry,
);

pub fn render_demo(value: &str) -> AnyElement {
    div()
        .px_3()
        .py_2()
        .rounded_md()
        .border_1()
        .border_color(rgb(0x22303f))
        .bg(rgb(0x111922))
        .child(value.to_string())
        .into_any_element()
}
