//! Frame-local host demo/mine/cheer residual log for GameWorld SetDemoMineCheer parity.

use super::ObjectId;
use std::cell::RefCell;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HostDemoMineCheerEvent {
    pub object: ObjectId,
    pub demo_suicided_detonating: bool,
    pub has_mine_data: bool,
    pub cheer_timer: f32,
}

thread_local! {
    static LOG: RefCell<Vec<HostDemoMineCheerEvent>> = RefCell::new(Vec::new());
}

pub fn record(
    object: ObjectId,
    demo_suicided_detonating: bool,
    has_mine_data: bool,
    cheer_timer: f32,
) {
    LOG.with(|log| {
        log.borrow_mut().push(HostDemoMineCheerEvent {
            object,
            demo_suicided_detonating,
            has_mine_data,
            cheer_timer,
        });
    });
}

pub fn drain() -> Vec<HostDemoMineCheerEvent> {
    LOG.with(|log| std::mem::take(&mut *log.borrow_mut()))
}

pub fn clear() {
    LOG.with(|log| log.borrow_mut().clear());
}
