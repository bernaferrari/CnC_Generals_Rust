//! Frame-local host identity log for GameWorld SetIdentity parity.

use super::ObjectId;
use std::cell::RefCell;

#[derive(Debug, Clone, PartialEq)]
pub struct HostIdentityEvent {
    pub object: ObjectId,
    /// Host Object::name residual (presentation display_name).
    pub name: String,
    pub team_color: [f32; 4],
}

thread_local! {
    static LOG: RefCell<Vec<HostIdentityEvent>> = RefCell::new(Vec::new());
}

pub fn record(object: ObjectId, name: String, team_color: [f32; 4]) {
    LOG.with(|log| {
        log.borrow_mut().push(HostIdentityEvent {
            object,
            name,
            team_color,
        });
    });
}

pub fn drain() -> Vec<HostIdentityEvent> {
    LOG.with(|log| std::mem::take(&mut *log.borrow_mut()))
}

pub fn clear() {
    LOG.with(|log| log.borrow_mut().clear());
}
