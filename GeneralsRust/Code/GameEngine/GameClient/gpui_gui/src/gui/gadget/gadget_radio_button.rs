use gpui::{div, prelude::*, px, rgb, AnyElement};

use crate::gui::source_catalog::{GadgetKind, GadgetPort, GuiPortRecord};

pub const RECORD: GuiPortRecord = GuiPortRecord::new(
    "Gadget/GadgetRadioButton.cpp",
    "crate::gui::gadget::gadget_radio_button",
    "Gadget Radio Button",
    "Ports mutually-exclusive grouped selection behavior for radio gadgets.",
);

pub const PORT: GadgetPort = GadgetPort::new(
    &RECORD,
    "Radio Button",
    "Exclusive option selector.",
    "Single active option per group.",
    GadgetKind::RadioButton,
);

pub fn render_demo(options: &[&str], selected: &str) -> AnyElement {
    div()
        .flex()
        .flex_col()
        .gap_1()
        .children(options.iter().map(|label| {
            div()
                .flex()
                .gap_2()
                .items_center()
                .child(
                    div()
                        .size(px(16.))
                        .rounded_full()
                        .border_1()
                        .border_color(rgb(0x8dc0ff))
                        .bg(if *label == selected {
                            rgb(0x32567c)
                        } else {
                            rgb(0x101720)
                        }),
                )
                .child((*label).to_string())
        }))
        .into_any_element()
}
