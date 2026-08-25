//! Host scripts `impl GameLogic` split.
//! Submodules are grandchildren of `game_logic.rs`; `pub(in super::super)`
//! matches the previous `pub(super)` visibility on the flat file.
#![allow(unused_imports, non_snake_case)]

mod add_object_selection;
mod ambush_leaflet;
mod angry_mob_aurora;
mod eva_camera;
mod helix_radar;
mod move_ambient_audio;
mod production_eva;
mod rebuild_dozer;
mod saboteur_car_bomb;

mod scripts_camera;
mod special_power_strikes;
mod stealth_mines;
mod ui_production;
mod unit_commands;
