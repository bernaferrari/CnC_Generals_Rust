//! Frame-local China infantry horde status logs for GW shadow parity.

use super::ObjectId;
use std::cell::RefCell;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChinaInfantryHordeKind {
    RedGuard,
    TankHunter,
    Minigunner,
}

#[derive(Debug, Clone)]
pub struct ChinaInfantryHordeEvent {
    pub id: ObjectId,
    pub kind: ChinaInfantryHordeKind,
    pub now_horde: bool,
    pub was_horde: bool,
}

thread_local! {
    static EVENTS: RefCell<Vec<ChinaInfantryHordeEvent>> = RefCell::new(Vec::new());
}

pub fn record(ev: ChinaInfantryHordeEvent) {
    EVENTS.with(|l| l.borrow_mut().push(ev));
}

pub fn drain() -> Vec<ChinaInfantryHordeEvent> {
    EVENTS.with(|l| std::mem::take(&mut *l.borrow_mut()))
}

pub fn clear() {
    EVENTS.with(|l| l.borrow_mut().clear());
}
