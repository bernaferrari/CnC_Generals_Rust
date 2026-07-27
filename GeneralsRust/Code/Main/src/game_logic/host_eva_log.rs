//! Frame-local host EVA pulse log for presentation audio residual.
//!
//! C++ `TheEva->setShouldPlay` edges are recorded here so PresentationFrame can
//! emit snapshot EVA audio without dual-reading live GameLogic mid-render.

use gamelogic::helpers::EvaEvent;
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

/// Map C++ EvaEvent residual → presentation audio event name.
/// Fail-closed: Debug-stable `EVA_{Variant}` names (not Miles speech asset table).
pub fn eva_event_audio_name(event: EvaEvent) -> String {
    format!("EVA_{event:?}")
}

pub fn record(name: impl Into<String>) {
    LOG.with(|log| {
        log.borrow_mut().push(HostEvaEvent { name: name.into() });
    });
}

/// Wave 534: record from typed EvaEvent (covers full setShouldPlay matrix).
pub fn record_event(event: EvaEvent) {
    record(eva_event_audio_name(event));
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
