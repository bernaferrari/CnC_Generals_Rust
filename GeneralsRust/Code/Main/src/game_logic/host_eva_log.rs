//! Frame-local host EVA pulse log for presentation audio residual.
//!
//! C++ `TheEva->setShouldPlay` edges are recorded here so PresentationFrame can
//! emit snapshot EVA audio without dual-reading live GameLogic mid-render.

use std::cell::RefCell;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HostEvaEvent {
    /// Presentation/audio event name (e.g. `EVA_LowPower`).
    pub name: String,
}

thread_local! {
    static LOG: RefCell<Vec<HostEvaEvent>> = RefCell::new(Vec::new());
    static LAST_DRAIN: RefCell<Vec<HostEvaEvent>> = RefCell::new(Vec::new());
}

pub fn record(name: impl Into<String>) {
    LOG.with(|log| {
        log.borrow_mut().push(HostEvaEvent { name: name.into() });
    });
}

pub fn drain() -> Vec<HostEvaEvent> {
    let v = LOG.with(|log| std::mem::take(&mut *log.borrow_mut()));
    if !v.is_empty() {
        LAST_DRAIN.with(|last| *last.borrow_mut() = v.clone());
    }
    v
}

pub fn take_last_drain() -> Vec<HostEvaEvent> {
    let pending = drain();
    if !pending.is_empty() {
        return pending;
    }
    LAST_DRAIN.with(|last| std::mem::take(&mut *last.borrow_mut()))
}

pub fn clear() {
    LOG.with(|log| log.borrow_mut().clear());
    LAST_DRAIN.with(|last| last.borrow_mut().clear());
}
