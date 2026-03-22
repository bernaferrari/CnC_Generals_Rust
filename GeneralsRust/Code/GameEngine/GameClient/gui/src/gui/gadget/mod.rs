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
        GadgetKind::PushButton => {
            gadget_push_button::render("Launch", &gadget_push_button::PushButtonState::default())
        }
        GadgetKind::CheckBox => gadget_check_box::render(
            "Enable subtitles",
            &gadget_check_box::CheckBoxState::new(true),
        ),
        GadgetKind::RadioButton => gadget_radio_button::render(&[
            gadget_radio_button::RadioButtonState {
                label: "Low".to_string(),
                group: 0,
                screen: 0,
                selected: false,
                hilited: false,
                mouse_track: true,
            },
            gadget_radio_button::RadioButtonState {
                label: "Medium".to_string(),
                group: 0,
                screen: 0,
                selected: false,
                hilited: true,
                mouse_track: true,
            },
            gadget_radio_button::RadioButtonState {
                label: "High".to_string(),
                group: 0,
                screen: 0,
                selected: true,
                hilited: false,
                mouse_track: true,
            },
        ]),
        GadgetKind::HorizontalSlider => {
            let mut state = gadget_horizontal_slider::HorizontalSliderState::default();
            state.position = 70;
            gadget_horizontal_slider::render("Volume: 70%", &state)
        }
        GadgetKind::VerticalSlider => {
            let mut state = gadget_vertical_slider::VerticalSliderState::default();
            state.position = 58;
            gadget_vertical_slider::render(&state)
        }
        GadgetKind::ListBox => {
            let state = gadget_list_box::ListBoxState {
                entries: vec![
                    "Tournament Desert".to_string(),
                    "Forgotten Forest".to_string(),
                    "Defcon 6".to_string(),
                ],
                selected_row: Some(0),
                ..Default::default()
            };
            gadget_list_box::render(&state)
        }
        GadgetKind::ComboBox => {
            let state = gadget_combo_box::ComboBoxState {
                selected_text: "China".to_string(),
                entries: vec!["USA".to_string(), "China".to_string(), "GLA".to_string()],
                ..Default::default()
            };
            gadget_combo_box::render(&state)
        }
        GadgetKind::ProgressBar => gadget_progress_bar::render(
            "Build progress",
            &gadget_progress_bar::ProgressBarState::new(66),
        ),
        GadgetKind::StaticText => {
            gadget_static_text::render(&gadget_static_text::StaticTextState {
                label: "Mission Briefing".to_string(),
                body: "Destroy the enemy command center before reinforcements arrive.".to_string(),
                font_name: None,
            })
        }
        GadgetKind::TextEntry => {
            let state = gadget_text_entry::TextEntryState {
                text: "PlayerName_01".to_string(),
                secret_text: "*************".to_string(),
                ..Default::default()
            };
            gadget_text_entry::render(&state)
        }
        GadgetKind::TabControl => gadget_tab_control::render(
            &["General", "Units", "Support"],
            &gadget_tab_control::TabControlState {
                active_tab: 1,
                tab_count: 3,
                disabled: vec![false, false, false],
                ..Default::default()
            },
        ),
    }
}
