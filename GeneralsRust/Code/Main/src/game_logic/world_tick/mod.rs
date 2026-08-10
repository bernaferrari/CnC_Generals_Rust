//! Host tick `impl GameLogic` split.
//! Submodules are grandchildren of `game_logic.rs`; `pub(in super::super)`
//! matches the previous `pub(super)` visibility on the flat file.
#![allow(unused_imports, non_snake_case)]

mod step;
mod production;
mod presence;
mod movement;
mod ai;
mod physics;
mod teams;
mod crates;
mod mood;
mod attack;
mod shock;
mod airfield;
mod combat;
