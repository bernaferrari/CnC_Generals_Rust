//! The same plain observation contract crosses simulation and UI loans.
pub use game_engine::common::thing::receiving_module_content::{
    UpgradeTooltipObservation, UpgradeTooltipSource,
};
#[cfg(test)]
#[path = "upgrade_observer_tests.rs"]
mod tests;
