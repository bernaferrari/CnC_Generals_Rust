//! Host objects `impl GameLogic` — support behavior/state ownership seams.
//!
//! This directory is a structural split of the former `support_states.rs`.
//! The child modules retain the original C++ behavior boundaries and APIs.
#![allow(unused_imports, non_snake_case)]

mod contain_states;
mod guard_states;
mod heal_contain_tunnel;
mod special_abilities;
mod supply_repair_docks;
mod update;
