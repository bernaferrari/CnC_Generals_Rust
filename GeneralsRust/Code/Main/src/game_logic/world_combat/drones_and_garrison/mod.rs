//! Host combat `impl GameLogic` — containment and special-power residuals.
//!
//! Behavior stays grouped by its owning C++ subsystem; child modules add no
//! external interface beyond the inherent `GameLogic` methods they implement.
#![allow(unused_imports, non_snake_case)]

mod defector;
mod firepoints;
mod garrison;
mod neutron;
mod production_and_power;
mod special_powers;
mod support_residuals;
mod transport;

#[cfg(test)]
mod tests;
