//! Upgrade Template — GameLogic view of the canonical C++ `UpgradeTemplate`.
//!
//! The definition type lives in Common (`game_engine::common::system::upgrade`,
//! C++ Common/System/Upgrade.cpp). Only the `Player`-dependent members
//! (`calcTimeToBuild`, `calcCostToBuild`) live here, next to GameLogic's
//! `Player`.
//!
//! Original C++ Author: Colin Day, March 2002

use crate::common::*;

pub use game_engine::common::system::upgrade::{UpgradeTemplate, UpgradeType};

/// C++ `UpgradeTemplate` members that take a `Player*`.
pub trait UpgradeTemplatePlayerExt {
    /// C++ `UpgradeTemplate::calcTimeToBuild` (Upgrade.cpp:126-139).
    fn calc_time_to_build(&self, player: &Player) -> Int;
    /// C++ `UpgradeTemplate::calcCostToBuild` (Upgrade.cpp:144-150).
    fn calc_cost_to_build(&self, player: &Player) -> Int;
}

impl UpgradeTemplatePlayerExt for UpgradeTemplate {
    fn calc_time_to_build(&self, player: &Player) -> Int {
        #[cfg(any(debug_assertions, feature = "internal", feature = "allow_debug_cheats"))]
        if player.builds_instantly() {
            return 1;
        }
        let _ = player;

        const LOGICFRAMES_PER_SECOND: Real = 30.0;
        (self.get_build_time() * LOGICFRAMES_PER_SECOND) as Int
    }

    fn calc_cost_to_build(&self, _player: &Player) -> Int {
        self.get_cost()
    }
}
