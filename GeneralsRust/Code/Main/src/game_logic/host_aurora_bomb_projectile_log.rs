//! Frame-local Aurora bomb projectile arrival/stale destroy logs for GW shadow parity.

use super::ObjectId;
use std::cell::RefCell;

#[derive(Debug, Clone)]
pub struct AuroraBombProjectileDestroyEvent {
    pub id: ObjectId,
    /// Snap position residual (aim) when arriving; None keeps current.
    pub snap_aim: Option<[f32; 3]>,
}

thread_local! {
    static DESTROY: RefCell<Vec<AuroraBombProjectileDestroyEvent>> = RefCell::new(Vec::new());
}

pub fn record_destroy(ev: AuroraBombProjectileDestroyEvent) {
    DESTROY.with(|l| l.borrow_mut().push(ev));
}

pub fn drain_destroys() -> Vec<AuroraBombProjectileDestroyEvent> {
    DESTROY.with(|l| std::mem::take(&mut *l.borrow_mut()))
}

pub fn clear() {
    DESTROY.with(|l| l.borrow_mut().clear());
}
