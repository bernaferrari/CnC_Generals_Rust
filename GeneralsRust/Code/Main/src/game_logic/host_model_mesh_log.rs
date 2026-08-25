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

/// Move the queue out for a whole-world staging boundary without dropping the
/// active world's pending presentation mutations.
pub(crate) fn take_for_world_stage() -> Vec<HostModelMeshEvent> {
    drain()
}

/// Restore or install the queue owned by a whole-world staging boundary.
pub(crate) fn replace_for_world_stage(next: Vec<HostModelMeshEvent>) -> Vec<HostModelMeshEvent> {
    LOG.with(|log| std::mem::replace(&mut *log.borrow_mut(), next))
}

pub fn clear() {
    LOG.with(|log| log.borrow_mut().clear());
}

pub fn len() -> usize {
    LOG.with(|log| log.borrow().len())
}
