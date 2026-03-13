pub mod gadget_check_box;
pub mod gadget_combo_box;
pub mod gadget_horizontal_slider;
pub mod gadget_list_box;
pub mod gadget_progress_bar;
pub mod gadget_push_button;
pub mod gadget_radio_button;
pub mod gadget_static_text;
pub mod gadget_tab_control;
pub mod gadget_text_entry;
pub mod gadget_vertical_slider;

use crate::gui::source_catalog::{GadgetKind, GadgetPort, GuiPortRecord};

pub fn records() -> Vec<&'static GuiPortRecord> {
    vec![
        &gadget_check_box::RECORD,
        &gadget_combo_box::RECORD,
        &gadget_horizontal_slider::RECORD,
        &gadget_list_box::RECORD,
        &gadget_progress_bar::RECORD,
        &gadget_push_button::RECORD,
        &gadget_radio_button::RECORD,
        &gadget_static_text::RECORD,
        &gadget_tab_control::RECORD,
        &gadget_text_entry::RECORD,
        &gadget_vertical_slider::RECORD,
    ]
}

pub fn ports() -> &'static [GadgetPort] {
    &[
        gadget_push_button::PORT,
        gadget_check_box::PORT,
        gadget_radio_button::PORT,
        gadget_horizontal_slider::PORT,
        gadget_vertical_slider::PORT,
        gadget_list_box::PORT,
        gadget_combo_box::PORT,
        gadget_progress_bar::PORT,
        gadget_static_text::PORT,
        gadget_text_entry::PORT,
        gadget_tab_control::PORT,
    ]
}

pub fn render_port(port: &GadgetPort) -> gpui::AnyElement {
    match port.kind {
        GadgetKind::PushButton => gadget_push_button::render_demo("Launch"),
        GadgetKind::CheckBox => gadget_check_box::render_demo("Enable subtitles", true),
        GadgetKind::RadioButton => {
            gadget_radio_button::render_demo(&["Low", "Medium", "High"], "High")
        }
        GadgetKind::HorizontalSlider => gadget_horizontal_slider::render_demo("Volume: 70%", 0.7),
        GadgetKind::VerticalSlider => gadget_vertical_slider::render_demo(0.58),
        GadgetKind::ListBox => gadget_list_box::render_demo(
            &["Tournament Desert", "Forgotten Forest", "Defcon 6"],
            "Tournament Desert",
        ),
        GadgetKind::ComboBox => gadget_combo_box::render_demo("China"),
        GadgetKind::ProgressBar => gadget_progress_bar::render_demo("Build progress", 0.66),
        GadgetKind::StaticText => gadget_static_text::render_demo(
            "Mission Briefing",
            "Destroy the enemy command center before reinforcements arrive.",
        ),
        GadgetKind::TextEntry => gadget_text_entry::render_demo("PlayerName_01"),
        GadgetKind::TabControl => {
            gadget_tab_control::render_demo(&["General", "Units", "Support"], "Units")
        }
    }
}
