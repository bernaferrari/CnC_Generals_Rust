//! Frame-local host rebuild/producer residual for GameWorld SetRebuildProducer parity.

use super::ObjectId;
use std::cell::RefCell;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HostRebuildProducerEvent {
    pub object: ObjectId,
    pub is_rebuild_hole: bool,
    pub rebuild_template_name: String,
    pub rebuild_ready_frame: u32,
    pub rebuild_spawner_id: Option<u32>,
    pub rebuild_worker_id: Option<u32>,
    pub rebuild_reconstructing_id: Option<u32>,
    pub producer_id: Option<u32>,
    pub construction_complete_clear_frame: u32,
}

thread_local! {
    static LOG: RefCell<Vec<HostRebuildProducerEvent>> = RefCell::new(Vec::new());
}

pub fn record(
    object: ObjectId,
    is_rebuild_hole: bool,
    rebuild_template_name: String,
    rebuild_ready_frame: u32,
    rebuild_spawner_id: Option<u32>,
    rebuild_worker_id: Option<u32>,
    rebuild_reconstructing_id: Option<u32>,
    producer_id: Option<u32>,
    construction_complete_clear_frame: u32,
) {
    LOG.with(|log| {
        log.borrow_mut().push(HostRebuildProducerEvent {
            object,
            is_rebuild_hole,
            rebuild_template_name,
            rebuild_ready_frame,
            rebuild_spawner_id,
            rebuild_worker_id,
            rebuild_reconstructing_id,
            producer_id,
            construction_complete_clear_frame,
        });
    });
}

pub fn drain() -> Vec<HostRebuildProducerEvent> {
    LOG.with(|log| std::mem::take(&mut *log.borrow_mut()))
}

pub fn clear() {
    LOG.with(|log| log.borrow_mut().clear());
}
