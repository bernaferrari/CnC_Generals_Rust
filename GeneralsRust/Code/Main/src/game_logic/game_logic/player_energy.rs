//! Instance-owned C++ Energy counters and frame-scoped sabotage observations.
//! A sabotage deadline never overwrites production. Snapshot restore derives
//! these counters from admitted objects, just as Energy::xfer omits them.
use super::*;

impl Player {
    pub(crate) fn energy_sabotaged_at(&self, frame: u32) -> bool {
        frame < self.power_sabotaged_till_frame
    }

    /// Energy.cpp:40-47, including equality at the sabotage deadline.
    pub(crate) fn energy_production_at(&self, frame: u32) -> i32 {
        if self.energy_sabotaged_at(frame) {
            0
        } else {
            self.power_produced
        }
    }

    pub(crate) fn energy_available_at(&self, frame: u32) -> i32 {
        self.energy_production_at(frame)
            .wrapping_sub(self.power_consumed)
    }

    /// Sabotage is insufficient even with zero consumption (Energy.cpp:68-77).
    pub(crate) fn has_sufficient_energy_at(&self, frame: u32) -> bool {
        !self.energy_sabotaged_at(frame) && self.power_produced >= self.power_consumed
    }

    pub(crate) fn energy_supply_ratio_at(&self, frame: u32) -> f32 {
        if self.energy_sabotaged_at(frame) {
            return 0.0;
        }
        if self.power_consumed == 0 {
            self.power_produced as f32
        } else {
            self.power_produced as f32 / self.power_consumed as f32
        }
    }

    pub(crate) fn refresh_energy_projection_at(&mut self, frame: u32) {
        self.power_available = self.energy_available_at(frame);
    }

    /// Energy::objectEnteringInfluence: every nonzero signed template delta
    /// signals onPowerBrownOutChange, even when sufficiency has not changed.
    pub(in crate::game_logic) fn add_template_energy(
        &mut self,
        energy: i32,
        frame: u32,
    ) -> Option<bool> {
        if energy == 0 {
            return None;
        }
        if energy > 0 {
            self.power_produced = self.power_produced.wrapping_add(energy);
        } else {
            self.power_consumed = self.power_consumed.wrapping_sub(energy);
        }
        self.refresh_energy_projection_at(frame);
        Some(!self.has_sufficient_energy_at(frame))
    }

    /// Energy::objectLeavingInfluence, called AFTER old Team removal while
    /// its old Player still exists. The caller must immediately admit the
    /// returned onPowerBrownOutChange value; this is not a deferred effect.
    /// Construction/disabled-positive-producer exclusion belongs to Object's
    /// caller and must happen before this arithmetic operation.
    pub(in crate::game_logic) fn remove_template_energy(
        &mut self,
        energy: i32,
        frame: u32,
    ) -> Option<bool> {
        if energy == 0 {
            return None;
        }
        if energy > 0 {
            self.power_produced = self.power_produced.wrapping_sub(energy);
        } else {
            self.power_consumed = self.power_consumed.wrapping_add(energy);
        }
        self.refresh_energy_projection_at(frame);
        Some(!self.has_sufficient_energy_at(frame))
    }
}

#[cfg(test)]
#[path = "player_energy_tests.rs"]
mod tests;
