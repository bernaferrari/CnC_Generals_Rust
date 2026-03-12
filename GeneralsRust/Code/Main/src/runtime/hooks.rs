/// Runtime attachment hook registry mirroring the C++ WW3D event system.
use std::ops::Deref;
use std::sync::{Arc, RwLock};

use log::trace;
use ww3d_renderer_3d::AttachmentRecord;

pub trait AttachmentListener: Send + Sync {
    fn on_attachment(&self, record: &AttachmentRecord);
}

impl<T> AttachmentListener for T
where
    T: Fn(&AttachmentRecord) + Send + Sync,
{
    fn on_attachment(&self, record: &AttachmentRecord) {
        self(record);
    }
}

#[derive(Clone, Default)]
pub struct AttachmentHookRegistry {
    listeners: Arc<RwLock<Vec<Arc<dyn AttachmentListener>>>>,
}

impl AttachmentHookRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register_listener<L>(&self, listener: L)
    where
        L: AttachmentListener + 'static,
    {
        let mut guard = self
            .listeners
            .write()
            .expect("attachment registry poisoned");
        guard.push(Arc::new(listener));
    }

    pub fn dispatch(&self, record: &AttachmentRecord) {
        let guard = self.listeners.read().expect("attachment registry poisoned");
        if guard.is_empty() {
            trace!(
                "Attachment dispatch with no listeners: {} (parent {})",
                record.name,
                record.parent_label
            );
        }
        for listener in guard.deref() {
            listener.on_attachment(record);
        }
    }
}

lazy_static::lazy_static! {
    pub static ref ATTACHMENT_HOOKS: AttachmentHookRegistry = AttachmentHookRegistry::new();
}
