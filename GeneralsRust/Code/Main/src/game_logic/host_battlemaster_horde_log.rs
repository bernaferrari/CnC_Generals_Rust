//! Frame-local Battlemaster horde status logs for GW shadow parity.

use super::ObjectId;
use std::cell::RefCell;

#[derive(Debug, Clone)]
pub struct BattlemasterHordeEvent {
    pub id: ObjectId,
    pub now_horde: bool,
    pub was_horde: bool,
}

thread_local! {
    static EVENTS: RefCell<Vec<BattlemasterHordeEvent>> = RefCell::new(Vec::new());
}

pub fn record(ev: BattlemasterHordeEvent) {
    EVENTS.with(|l| l.borrow_mut().push(ev));
}

pub fn drain() -> Vec<BattlemasterHordeEvent> {
    EVENTS.with(|l| std::mem::take(&mut *l.borrow_mut()))
}

pub fn clear() {
    EVENTS.with(|l| l.borrow_mut().clear());
}
