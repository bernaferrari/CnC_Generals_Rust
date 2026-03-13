pub mod control_bar;
pub mod control_bar_beacon;
pub mod control_bar_command;
pub mod control_bar_command_processing;
pub mod control_bar_multi_select;
pub mod control_bar_observer;
pub mod control_bar_ocl_timer;
pub mod control_bar_print_positions;
pub mod control_bar_resizer;
pub mod control_bar_scheme;
pub mod control_bar_structure_inventory;
pub mod control_bar_under_construction;
pub mod scene;

use crate::gui::source_catalog::{ControlBarPort, GuiPortRecord};

pub fn records() -> Vec<&'static GuiPortRecord> {
    vec![
        &control_bar::RECORD,
        &control_bar_beacon::RECORD,
        &control_bar_command::RECORD,
        &control_bar_command_processing::RECORD,
        &control_bar_multi_select::RECORD,
        &control_bar_ocl_timer::RECORD,
        &control_bar_observer::RECORD,
        &control_bar_print_positions::RECORD,
        &control_bar_resizer::RECORD,
        &control_bar_scheme::RECORD,
        &control_bar_structure_inventory::RECORD,
        &control_bar_under_construction::RECORD,
    ]
}

pub fn ports() -> &'static [ControlBarPort] {
    &[
        control_bar::PORT,
        control_bar_command::PORT,
        control_bar_command_processing::PORT,
        control_bar_structure_inventory::PORT,
        control_bar_under_construction::PORT,
        control_bar_beacon::PORT,
        control_bar_multi_select::PORT,
        control_bar_observer::PORT,
        control_bar_ocl_timer::PORT,
        control_bar_resizer::PORT,
        control_bar_scheme::PORT,
        control_bar_print_positions::PORT,
    ]
}

pub fn render_port(port: &ControlBarPort) -> gpui::AnyElement {
    scene::render_port(port)
}
