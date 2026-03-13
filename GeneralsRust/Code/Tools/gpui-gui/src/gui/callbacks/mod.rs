pub mod control_bar_callback;
pub mod control_bar_popup_description;
pub mod diplomacy;
pub mod extended_message_box;
pub mod generals_exp_points;
pub mod ime_candidate;
pub mod in_game_chat;
pub mod in_game_popup_message;
pub mod menus;
pub mod message_box;
pub mod replay_controls;
pub mod scene;

use crate::gui::source_catalog::{CallbackPort, GuiPortRecord};

pub fn records() -> Vec<&'static GuiPortRecord> {
    let mut records = vec![
        &control_bar_callback::RECORD,
        &control_bar_popup_description::RECORD,
        &diplomacy::RECORD,
        &extended_message_box::RECORD,
        &generals_exp_points::RECORD,
        &ime_candidate::RECORD,
        &in_game_chat::RECORD,
        &in_game_popup_message::RECORD,
        &message_box::RECORD,
        &replay_controls::RECORD,
    ];
    records.extend(menus::records());
    records
}

pub fn ports() -> &'static [CallbackPort] {
    &[
        control_bar_callback::PORT,
        control_bar_popup_description::PORT,
        diplomacy::PORT,
        extended_message_box::PORT,
        generals_exp_points::PORT,
        ime_candidate::PORT,
        in_game_chat::PORT,
        in_game_popup_message::PORT,
        message_box::PORT,
        replay_controls::PORT,
    ]
}

pub fn render_port(port: &CallbackPort) -> gpui::AnyElement {
    scene::render_port(port)
}
