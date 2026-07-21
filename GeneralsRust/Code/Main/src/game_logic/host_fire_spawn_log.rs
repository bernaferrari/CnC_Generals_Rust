//! Frame-local host weapon fire-spawn log for GameWorld fire-spawn authority.
//!
//! When `GENERALS_GAMEWORLD_FIRE_SPAWN_AUTHORITY` is on, `queue_projectile` only
//! records here; shadow applies spawns into host CombatSystem before projectile
//! integrate authority runs.

use super::combat::PendingProjectile;
use std::cell::RefCell;

thread_local! {
    static LOG: RefCell<Vec<PendingProjectile>> = RefCell::new(Vec::new());
}

pub fn record(pending: PendingProjectile) {
    LOG.with(|log| log.borrow_mut().push(pending));
}

pub fn drain() -> Vec<PendingProjectile> {
    LOG.with(|log| std::mem::take(&mut *log.borrow_mut()))
}

pub fn clear() {
    LOG.with(|log| log.borrow_mut().clear());
}
