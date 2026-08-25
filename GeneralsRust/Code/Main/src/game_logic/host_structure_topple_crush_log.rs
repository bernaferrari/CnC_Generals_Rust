//! Frame-local StructureTopple crush-sweep log for GameWorld shadow parity.
//!
//! Under coupled dual-tick, GW sole-ticks StructureToppleUpdate and emits
//! applyCrushingDamage residual samples here so host can apply without
//! dual-advancing last_crushed_location.

use super::ObjectId;
use super::host_structure_topple::StructureToppleCrushSample;
use std::cell::RefCell;

thread_local! {
    static LOG: RefCell<Vec<(ObjectId, Vec<StructureToppleCrushSample>)>> =
        RefCell::new(Vec::new());
}

pub fn record(object: ObjectId, samples: Vec<StructureToppleCrushSample>) {
    if samples.is_empty() {
        return;
    }
    LOG.with(|log| log.borrow_mut().push((object, samples)));
}

pub fn drain() -> Vec<(ObjectId, Vec<StructureToppleCrushSample>)> {
    LOG.with(|log| std::mem::take(&mut *log.borrow_mut()))
}

pub fn clear() {
    LOG.with(|log| log.borrow_mut().clear());
}
