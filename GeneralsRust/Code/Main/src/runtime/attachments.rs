use crate::runtime::hooks::ATTACHMENT_HOOKS;
use log::{trace, warn};
use std::collections::VecDeque;
use std::sync::{Mutex, OnceLock};
use ww3d_renderer_3d::AttachmentRecord;

pub struct AttachmentDispatcher;
const MAX_PENDING_ATTACHMENTS: usize = 4096;

fn pending_attachment_queue() -> &'static Mutex<VecDeque<AttachmentRecord>> {
    static QUEUE: OnceLock<Mutex<VecDeque<AttachmentRecord>>> = OnceLock::new();
    QUEUE.get_or_init(|| Mutex::new(VecDeque::new()))
}

impl AttachmentDispatcher {
    pub fn dispatch(records: Vec<AttachmentRecord>) {
        let mut queued = pending_attachment_queue()
            .lock()
            .expect("attachment queue poisoned");

        for record in records {
            trace!(
                "Attachment generated: {} (parent {})",
                record.name,
                record.parent_label
            );
            ATTACHMENT_HOOKS.dispatch(&record);
            if queued.len() >= MAX_PENDING_ATTACHMENTS {
                warn!(
                    "Attachment queue overflow (>{}), dropping oldest event",
                    MAX_PENDING_ATTACHMENTS
                );
                let _ = queued.pop_front();
            }
            queued.push_back(record);
        }
    }

    /// Drain attachment records emitted this frame for gameplay-side processing.
    pub fn drain_pending() -> Vec<AttachmentRecord> {
        let mut queued = pending_attachment_queue()
            .lock()
            .expect("attachment queue poisoned");
        queued.drain(..).collect()
    }
}
