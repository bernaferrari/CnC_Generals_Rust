//! Host tick `impl GameLogic` split.
//! Submodules are grandchildren of `game_logic.rs`; `pub(in super::super)`
//! matches the previous `pub(super)` visibility on the flat file.
#![allow(unused_imports, non_snake_case)]

mod ai;
mod airfield;
mod attack;
mod collide_dispatch;
mod collide_modules;

mod combat;
mod combat_fire_fx;
mod crates;
mod mood;
mod movement;
mod physics;
mod presence;
mod production;
mod shock;
mod sleepy;
mod step;
pub(in super::super) use sleepy::{HostSleepyHeap, HostSleepyKind};
mod teams;
