use crate::runtime::hooks::ATTACHMENT_HOOKS;
use log::trace;
use ww3d_renderer_3d::AttachmentRecord;

/// Deliver renderer attachments in emission order. No caller ever drained the
/// former process-wide backlog, so retaining records after delivery only let
/// one match's render events leak into the next match.
pub fn dispatch_attachments(records: Vec<AttachmentRecord>) {
    for record in records {
        trace!(
            "Attachment generated: {} (parent {})",
            record.name, record.parent_label
        );
        ATTACHMENT_HOOKS.dispatch(&record);
    }
}
