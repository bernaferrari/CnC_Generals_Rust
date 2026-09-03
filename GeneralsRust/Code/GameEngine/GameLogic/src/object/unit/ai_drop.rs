//! UnitAIUpdate Drop — clear pathfinder goal cells like C++.

#![allow(unused_imports)]

use super::ai_core::UnitAIUpdate;
use super::imports::*;
use super::registry::get_unit_arc;

impl Drop for UnitAIUpdate {
    fn drop(&mut self) {
        let Some(unit) = get_unit_arc(self.unit_id) else {
            return;
        };
        let Ok(guard) = unit.read() else {
            return;
        };
        // Object may already be unregistered during teardown; never panic in Drop.
        let Some(base) = guard.get_base_object() else {
            return;
        };
        let owner_id = base
            .read()
            .ok()
            .map(|obj| obj.get_id())
            .unwrap_or(INVALID_ID);
        let is_immobile = base
            .read()
            .ok()
            .map(|obj| obj.is_kind_of(KindOf::Immobile))
            .unwrap_or(false);
        if is_immobile {
            return;
        }
        let (radius, center_in_cell) = Self::compute_pathfind_radius_and_center(&guard);
        drop(guard);

        let ai_store = the_ai(); if let Ok(ai_lock) = ai_store.read() {
            if let Some(pathfinder) = ai_lock.pathfinder() {
                if let Ok(mut pf_guard) = pathfinder.write() {
                    self.remove_goal_cells(&mut pf_guard, owner_id, radius, center_in_cell);
                }
            }
        }
    }
}
