//! Frame-local host model mesh log for GameWorld SetModelMesh parity.

use super::ObjectId;
use std::cell::RefCell;

#[derive(Debug, Clone, PartialEq)]
pub struct HostModelMeshEvent {
    pub object: ObjectId,
    pub model_key: String,
    pub mesh_scale: f32,
}

thread_local! {
    static LOG: RefCell<Vec<HostModelMeshEvent>> = RefCell::new(Vec::new());
}

pub fn record(object: ObjectId, model_key: impl Into<String>, mesh_scale: f32) {
    LOG.with(|log| {
        log.borrow_mut().push(HostModelMeshEvent {
            object,
            model_key: model_key.into(),
            mesh_scale,
        });
    });
}

pub fn drain() -> Vec<HostModelMeshEvent> {
    LOG.with(|log| std::mem::take(&mut *log.borrow_mut()))
}

pub fn clear() {
    LOG.with(|log| log.borrow_mut().clear());
}
