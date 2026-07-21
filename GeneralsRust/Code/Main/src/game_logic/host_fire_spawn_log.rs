//! Frame-local host weapon fire-spawn log for GameWorld fire-spawn authority.
//!
//! When `GENERALS_GAMEWORLD_FIRE_SPAWN_AUTHORITY` is on, `queue_projectile` only
//! records here; shadow applies spawns into host CombatSystem before projectile
//! integrate authority runs.
//!
//! Residual auto-fire also records hitscan pairs when same-frame host HP damage
//! already applied, so shadow can zero projectile damage and avoid double-dip.

use super::combat::PendingProjectile;
use crate::game_logic::ObjectId;
use std::cell::RefCell;

thread_local! {
    static LOG: RefCell<Vec<PendingProjectile>> = RefCell::new(Vec::new());
    /// Residual auto-fire pairs that already took host hitscan damage this frame.
    static RESIDUAL_HITSCAN: RefCell<Vec<(ObjectId, ObjectId)>> = RefCell::new(Vec::new());
}

pub fn record(pending: PendingProjectile) {
    LOG.with(|log| log.borrow_mut().push(pending));
}

pub fn drain() -> Vec<PendingProjectile> {
    LOG.with(|log| std::mem::take(&mut *log.borrow_mut()))
}

pub fn clear() {
    LOG.with(|log| log.borrow_mut().clear());
    RESIDUAL_HITSCAN.with(|h| h.borrow_mut().clear());
}

/// Residual auto-fire applied host hitscan damage for this shooter→target pair.
pub fn record_residual_hitscan(shooter: ObjectId, target: ObjectId) {
    RESIDUAL_HITSCAN.with(|h| h.borrow_mut().push((shooter, target)));
}

/// Drain residual hitscan pairs (consumed when shadow applies fire-spawns).
pub fn drain_residual_hitscans() -> Vec<(ObjectId, ObjectId)> {
    RESIDUAL_HITSCAN.with(|h| std::mem::take(&mut *h.borrow_mut()))
}
