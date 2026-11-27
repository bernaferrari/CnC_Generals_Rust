//! Authored timed water changes, advanced once per terrain phase.

use crate::time::{LOGIC_FRAMES_PER_SECOND, LOGIC_HZ};

pub const MAX_DYNAMIC_WATER_ENTRIES: usize = 64;
pub const WATER_GRID_NAME_CPP: &str = "Water Grid";

pub fn is_grid_water_name(name: &str) -> bool {
    name.eq_ignore_ascii_case(WATER_GRID_NAME_CPP) || name.eq_ignore_ascii_case("GridWater")
}

/// The legacy editor name is kept alongside the trigger ID because old saves
/// and grid water can resolve by name alone.
#[derive(Debug, Clone)]
pub struct WaterTransition<Name> {
    pub trigger_id: i32,
    pub water_name: Name,
    pub change_per_frame: f32,
    pub target_height: f32,
    pub damage_amount: f32,
    pub current_height: f32,
}

#[derive(Debug)]
pub struct WaterTransitionStep<Name> {
    pub trigger_id: i32,
    pub name: Name,
    pub height: f32,
    pub damage: f32,
    pub terminal: bool,
}

/// Retains the receiving world's original swap-removal and save order.
#[derive(Debug)]
pub struct WaterTransitions<Name> {
    entries: Vec<WaterTransition<Name>>,
}

impl<Name> Default for WaterTransitions<Name> {
    fn default() -> Self {
        Self {
            entries: Vec::new(),
        }
    }
}

impl<Name: AsRef<str> + Clone> WaterTransitions<Name> {
    pub fn is_full(&self) -> bool {
        self.entries.len() >= MAX_DYNAMIC_WATER_ENTRIES
    }

    pub fn clear(&mut self) {
        self.entries.clear();
    }

    pub fn entries(&self) -> &[WaterTransition<Name>] {
        &self.entries
    }

    /// C++ TerrainLogic::changeWaterHeightOverTime. Zero time still advances
    /// only inside the next terrain phase.
    pub fn schedule(
        &mut self,
        trigger_id: i32,
        water_name: Name,
        current_height: f32,
        target_height: f32,
        transition_time_seconds: f32,
        damage_amount: f32,
    ) {
        if self.is_full() {
            return;
        }
        let mut index = 0;
        while index < self.entries.len() {
            let entry = &self.entries[index];
            let same = if trigger_id >= 0 {
                entry.trigger_id == trigger_id
            } else {
                is_grid_water_name(entry.water_name.as_ref())
            };
            if same {
                self.entries.swap_remove(index);
            } else {
                index += 1;
            }
        }
        self.entries.push(WaterTransition {
            trigger_id,
            water_name,
            change_per_frame: (target_height - current_height)
                / (LOGIC_FRAMES_PER_SECOND * transition_time_seconds),
            target_height,
            damage_amount,
            current_height,
        });
    }

    pub fn from_snapshot(
        entries: Vec<WaterTransition<Name>>,
        mut validate: impl FnMut(&mut WaterTransition<Name>) -> Result<(), String>,
    ) -> Result<Self, String> {
        let mut candidate = Vec::with_capacity(entries.len().min(MAX_DYNAMIC_WATER_ENTRIES));
        for mut entry in entries {
            if candidate.len() >= MAX_DYNAMIC_WATER_ENTRIES {
                return Err(format!(
                    "TerrainLogic::restore_dynamic_water_entries exceeds max dynamic entries ({})",
                    MAX_DYNAMIC_WATER_ENTRIES
                ));
            }
            validate(&mut entry)?;
            candidate.push(entry);
        }
        Ok(Self { entries: candidate })
    }

    /// Float accumulator continues even when integer polygon Z plateaus.
    /// The driver applies reclassification and damage before terminal removal.
    pub fn advance(&mut self, index: usize, frame: u32) -> Option<WaterTransitionStep<Name>> {
        let entry = self.entries.get_mut(index)?;
        let next = entry.current_height + entry.change_per_frame;
        let terminal = if entry.change_per_frame > 0.0 {
            next >= entry.target_height
        } else {
            next <= entry.target_height
        };
        let height = if terminal { entry.target_height } else { next };
        let damage = if terminal || frame % LOGIC_HZ == 0 {
            entry.damage_amount
        } else {
            0.0
        };
        if !terminal {
            entry.current_height = next;
        }
        Some(WaterTransitionStep {
            trigger_id: entry.trigger_id,
            name: entry.water_name.clone(),
            height,
            damage,
            terminal,
        })
    }

    pub fn finish(&mut self, index: usize, terminal: bool) {
        if terminal && index < self.entries.len() {
            self.entries.swap_remove(index);
        }
    }
}
