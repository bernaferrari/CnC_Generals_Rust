//! Host scripts `impl GameLogic` split.
//! Submodules are grandchildren of `game_logic.rs`; `pub(in super::super)`
//! matches the previous `pub(super)` visibility on the flat file.
#![allow(unused_imports, non_snake_case)]

mod add_object_selection;
mod ambush_leaflet;
mod angry_mob_aurora;
mod camera_admission;
mod camera_player_defaults;
mod camera_player_input;
mod eva_camera;
mod helix_radar;
mod instant_build_completion;
mod move_ambient_audio;
mod production_eva;
mod rebuild_dozer;
mod saboteur_car_bomb;

mod scripts_camera;
mod special_power_strikes;
mod stealth_mines;
mod ui_production;
mod unit_commands;

#[cfg(test)]
mod authored_refund_tests;

#[cfg(test)]
mod capture_callback_owner_tests;

#[cfg(test)]
mod construction_command_ui_tests;

#[cfg(test)]
mod cold_grid_owner_tests;

#[cfg(test)]
mod voice_fear_owner_tests;

#[cfg(test)]
mod movement_audio_owner_tests;

mod base_ai_machine_owner;
#[cfg(test)]
mod base_ai_machine_owner_tests;
mod derived_ai_idle;
#[cfg(test)]
mod derived_ai_idle_tests;
