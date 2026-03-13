pub mod animate_window_manager;
pub mod callbacks;
pub mod challenge_generals;
pub mod control_bar;
pub mod disconnect_menu;
pub mod establish_connections_menu;
pub mod gadget;
pub mod game_font;
pub mod game_window;
pub mod game_window_global;
pub mod game_window_manager;
pub mod game_window_manager_script;
pub mod game_window_transitions;
pub mod game_window_transitions_styles;
pub mod header_template;
pub mod ime_manager;
pub mod load_screen;
pub mod process_animate_window;
pub mod shell;
pub mod source_catalog;
pub mod system_scene;
pub mod win_instance_data;
pub mod window_layout;
pub mod window_video_manager;

use self::source_catalog::GuiPortRecord;

pub fn all_records() -> Vec<&'static GuiPortRecord> {
    let mut records = vec![
        &animate_window_manager::RECORD,
        &challenge_generals::RECORD,
        &disconnect_menu::RECORD,
        &establish_connections_menu::RECORD,
        &game_font::RECORD,
        &game_window::RECORD,
        &game_window_global::RECORD,
        &game_window_manager::RECORD,
        &game_window_manager_script::RECORD,
        &game_window_transitions::RECORD,
        &game_window_transitions_styles::RECORD,
        &header_template::RECORD,
        &ime_manager::RECORD,
        &load_screen::RECORD,
        &process_animate_window::RECORD,
        &win_instance_data::RECORD,
        &window_layout::RECORD,
        &window_video_manager::RECORD,
    ];
    records.extend(shell::records());
    records.extend(gadget::records());
    records.extend(control_bar::records());
    records.extend(callbacks::records());
    records
}

pub fn find_record(cpp_relative_path: &str) -> Option<&'static GuiPortRecord> {
    all_records()
        .into_iter()
        .find(|record| record.cpp_relative_path == cpp_relative_path)
}

pub fn groups() -> Vec<&'static str> {
    let mut groups = Vec::new();
    for record in all_records() {
        let group = group_for_path(record.cpp_relative_path);
        if !groups.contains(&group) {
            groups.push(group);
        }
    }
    groups
}

pub fn records_in_group(group: &str) -> Vec<&'static GuiPortRecord> {
    all_records()
        .into_iter()
        .filter(|record| group_for_path(record.cpp_relative_path) == group)
        .collect()
}

pub fn group_for_path(path: &'static str) -> &'static str {
    match path.split_once('/') {
        Some((group, _)) => group,
        None => "Core",
    }
}
