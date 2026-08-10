//! Host scripts `impl GameLogic` split.
//! Submodules are grandchildren of `game_logic.rs`; `pub(in super::super)`
//! matches the previous `pub(super)` visibility on the flat file.
#![allow(unused_imports, non_snake_case)]

mod saboteur_car_bomb;
mod helix_radar;
mod angry_mob_aurora;
mod stealth_mines;
mod special_power_strikes;
mod ambush_leaflet;
mod unit_commands;
mod add_object_selection;
mod scripts_camera;
mod ui_production;
mod production_eva;
mod rebuild_dozer;
mod eva_camera;
