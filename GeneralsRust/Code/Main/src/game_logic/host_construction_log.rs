//! Frame-local host structure construction-complete log for presentation/shadow.

use super::ObjectId;
use std::cell::RefCell;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HostConstructionEvent {
    pub id: ObjectId,
    pub template_name: String,
}

thread_local! {
    static LOG: RefCell<Vec<HostConstructionEvent>> = RefCell::new(Vec::new());
}

pub fn record(id: ObjectId, template_name: impl Into<String>) {
    LOG.with(|log| {
        log.borrow_mut().push(HostConstructionEvent {
            id,
            template_name: template_name.into(),
        });
    });
}

pub fn drain() -> Vec<HostConstructionEvent> {
    LOG.with(|log| std::mem::take(&mut *log.borrow_mut()))
}

pub fn clear() {
    LOG.with(|log| log.borrow_mut().clear());
}

pub fn snapshot() -> Vec<HostConstructionEvent> {
    LOG.with(|log| log.borrow().clone())
}
